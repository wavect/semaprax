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
    ServiceDatabaseAdapter, ServiceHostAdapterRequestV1, ServiceTelemetryAdapter,
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
    MAX_ACCOUNTS, MAX_STATE_BYTES,
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
/// The local reference profile's bounded session policy. Configuration bytes
/// never select clock policy.
pub const DEFAULT_SESSION_IDLE_SECONDS: u64 = 15 * 60;
pub const DEFAULT_SESSION_ABSOLUTE_SECONDS: u64 = 8 * 60 * 60;
pub const MAX_SESSION_LIFETIME_SECONDS: u64 = 7 * 24 * 60 * 60;

/// Stable refusal categories for binding intent to grants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindRefusal {
    /// Fixture-mode intent declares no capabilities and has no host runner.
    FixtureMode,
    /// The decoded requirements are not exactly the four host capabilities.
    IncompleteRequirements,
    /// The deployment binding is not a valid outbound identity.
    InvalidDeployment,
    /// The operator-selected session deadline window is not bounded or ordered.
    InvalidSessionPolicy,
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
    session_idle_seconds: u64,
    session_absolute_seconds: u64,
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
        session_idle_seconds: u64,
        session_absolute_seconds: u64,
    ) -> Result<Self, BindRefusal> {
        if !valid_identity(&deployment_binding) {
            return Err(BindRefusal::InvalidDeployment);
        }
        if session_idle_seconds > session_absolute_seconds
            || session_absolute_seconds > MAX_SESSION_LIFETIME_SECONDS
        {
            return Err(BindRefusal::InvalidSessionPolicy);
        }
        Ok(Self {
            state_directory,
            outbound_directory,
            secrets,
            deployment_binding,
            sync_mode,
            session_idle_seconds,
            session_absolute_seconds,
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
    session_idle_seconds: u64,
    session_absolute_seconds: u64,
}

/// Bind decoded host-mode intent to host grants and load the starting
/// state. Fixture-mode intent is refused: it has no runner here.
pub fn bind<'revision, 'directory>(
    intent: &ServiceHostAdapterRequestV1,
    decisions: DecisionEngine<'revision>,
    grants: HostGrants<'directory>,
    initial: InitialState,
) -> Result<(BoundHost<'revision, 'directory>, CommittedState), BindRefusal> {
    if intent.requirements().len() != 3 {
        if intent.requirements().is_empty() {
            return Err(BindRefusal::FixtureMode);
        }
        return Err(BindRefusal::IncompleteRequirements);
    }
    // The reference deployment binds the decoded snapshot profile to the
    // durable snapshot store. It accepts no SQL adapter or DSN.
    match intent.database().map(|database| database.adapter()) {
        Some(ServiceDatabaseAdapter::Snapshot) => {}
        None => return Err(BindRefusal::IncompleteRequirements),
    }
    // The decoded TLS listen origin is intent only: this host binds
    // loopback plaintext from an explicit operator grant (see `serve`).
    if intent.http().is_none() || intent.secrets().is_none() {
        return Err(BindRefusal::IncompleteRequirements);
    }
    let telemetry = intent
        .telemetry()
        .ok_or(BindRefusal::IncompleteRequirements)?;
    if telemetry.adapter() != ServiceTelemetryAdapter::SemapraxJsonEvents {
        return Err(BindRefusal::IncompleteRequirements);
    }
    let telemetry_origin = telemetry.endpoint_origin().to_owned();
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
            session_idle_seconds: grants.session_idle_seconds,
            session_absolute_seconds: grants.session_absolute_seconds,
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
    handle_with_clock(host, committed, exchange, &mut current_tick)
}

// Host-only clock seam: request/configuration bytes cannot provide a clock.
// Read lazily at the existing login/authentication boundary, so unauthenticated
// refusals and routes without time policy gain no clock dependency.
fn handle_with_clock(
    host: &mut BoundHost<'_, '_>,
    committed: &mut CommittedState,
    exchange: &HttpExchange,
    clock: &mut dyn FnMut() -> Option<u64>,
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
        return login(host, committed, exchange, clock);
    }
    // Only the closed set of protected route shapes reaches the session
    // boundary. Unknown paths must not turn a bearer-looking header into a
    // clock dependency or a durable session transition.
    if !route_needs_auth(&exchange.method, &exchange.target) {
        return error(404, "unknown_route", None);
    }
    let authenticated = match authenticate(host, committed, exchange, clock) {
        Authentication::Authenticated(value) => value,
        Authentication::Unauthorized => return error(401, "unauthorized", None),
        Authentication::Failed => return error(500, "decision_failed", None),
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

enum Authentication {
    Authenticated(Authenticated),
    Unauthorized,
    Failed,
}

fn route_needs_auth(method: &str, target: &str) -> bool {
    (method == "POST" && target == "/v1/logout")
        || (method == "POST" && target == "/v1/tasks")
        || (matches!(method, "GET" | "PATCH" | "DELETE")
            && task_member(target).is_some())
        || (method == "POST" && target == "/v1/jobs/enqueue")
        || (method == "POST" && job_member(target, "/complete").is_some())
        || (method == "GET" && job_member(target, "").is_some())
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
    host: &mut BoundHost<'_, '_>,
    committed: &mut CommittedState,
    exchange: &HttpExchange,
    clock: &mut dyn FnMut() -> Option<u64>,
) -> Authentication {
    let value = exchange
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("authorization"))
        .map(|(_, value)| value.as_str());
    let Some(token) = value.and_then(|value| value.strip_prefix("Bearer ")) else {
        return Authentication::Unauthorized;
    };
    let Some((id, tag)) = token.split_once('.') else {
        return Authentication::Unauthorized;
    };
    if id.len() != SESSION_ID_BYTES * 2
        || tag.len() != 64
        || !id.bytes().all(|byte| byte.is_ascii_hexdigit())
        || !tag.bytes().all(|byte| byte.is_ascii_hexdigit())
        || id.bytes().any(|byte| byte.is_ascii_uppercase())
        || tag.bytes().any(|byte| byte.is_ascii_uppercase())
    {
        return Authentication::Unauthorized;
    }
    let Some(id_bytes) = unhex(id) else {
        return Authentication::Unauthorized;
    };
    let Ok(mut mac) = HmacSha256::new_from_slice(host.secrets.session_key()) else {
        return Authentication::Failed;
    };
    mac.update(&id_bytes);
    let Some(tag) = unhex(tag) else {
        return Authentication::Unauthorized;
    };
    if mac.verify_slice(&tag).is_err() {
        return Authentication::Unauthorized;
    }
    let Some(session) = committed.state.session_by_id(id) else {
        return Authentication::Unauthorized;
    };
    let Some(now_tick) = clock() else {
        return Authentication::Failed;
    };
    let (state, account, idle_deadline_tick, absolute_deadline_tick) = (
        u64::from(session.state),
        session.account,
        match u64::try_from(session.idle_deadline_tick) {
            Ok(value) => value,
            Err(_) => return Authentication::Failed,
        },
        match u64::try_from(session.absolute_deadline_tick) {
            Ok(value) => value,
            Err(_) => return Authentication::Failed,
        },
    );
    let usable = match host.decisions.session_is_usable(
        state,
        now_tick,
        idle_deadline_tick,
        absolute_deadline_tick,
    ) {
        Ok(value) => value,
        Err(_) => return Authentication::Failed,
    };
    let next_state = match host.decisions.session_next_state_on_access(
        state,
        now_tick,
        idle_deadline_tick,
        absolute_deadline_tick,
    ) {
        Ok(value) if value <= 5 => value as u8,
        Ok(_) | Err(_) => return Authentication::Failed,
    };
    if usable != (next_state == 0) {
        return Authentication::Failed;
    }
    if next_state != session.state {
        let mut state = committed.state.clone();
        let Some(session) = state.sessions.iter_mut().find(|session| session.id == id) else {
            return Authentication::Failed;
        };
        session.state = next_state;
        if commit(host, committed, state).is_err() {
            return Authentication::Failed;
        }
    }
    if !usable {
        return Authentication::Unauthorized;
    }
    Authentication::Authenticated(Authenticated {
        account,
        session: id.to_owned(),
    })
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    let mut bytes = Vec::with_capacity(text.len() / 2);
    for pair in text.as_bytes().chunks(2) {
        bytes.push(u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?);
    }
    Some(bytes)
}

fn current_tick() -> Option<u64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
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
    let active_count = match u64::try_from(committed.state.accounts.len()) {
        Ok(count) => count,
        Err(_) => return error(500, "decision_failed", None),
    };
    let max_accounts = match u64::try_from(MAX_ACCOUNTS) {
        Ok(limit) => limit,
        Err(_) => return error(500, "decision_failed", None),
    };
    match host.decisions.registration_admitted(
        username.as_bytes(),
        active_count,
        max_accounts,
        u64::from(PASSWORD_MEMORY_KIB),
        u64::from(PASSWORD_ITERATIONS),
        u64::from(PASSWORD_PARALLELISM),
    ) {
        Ok(true) => {}
        Ok(false) => return error(400, "registration_not_admitted", Some(&committed.digest)),
        Err(_) => return error(500, "decision_failed", None),
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
    clock: &mut dyn FnMut() -> Option<u64>,
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
    let now_tick = match clock() {
        Some(tick) => tick,
        None => return error(500, "clock_unavailable", None),
    };
    let idle_deadline_tick = match i64::try_from(now_tick.saturating_add(host.session_idle_seconds))
    {
        Ok(tick) => tick,
        Err(_) => return error(500, "clock_unavailable", None),
    };
    let absolute_deadline_tick =
        match i64::try_from(now_tick.saturating_add(host.session_absolute_seconds)) {
            Ok(tick) => tick,
            Err(_) => return error(500, "clock_unavailable", None),
        };
    let mut state = committed.state.clone();
    state.sessions.push(Session {
        id,
        account: account_id,
        state: 0,
        idle_deadline_tick,
        absolute_deadline_tick,
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
    session.state = match host
        .decisions
        .session_next_state_on_logout(u64::from(session.state))
    {
        Ok(value) if value <= 5 => value as u8,
        Ok(_) | Err(_) => return error(500, "decision_failed", None),
    };
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
    // Task creation starts from the host's fixed idle transaction fact. The
    // checked source must admit it before this route derives an identifier,
    // constructs candidate state, or reaches the durable snapshot commit.
    match host.decisions.create_is_committed(0) {
        Ok(true) => {}
        Ok(false) => return error(403, "create_not_admitted", Some(&committed.digest)),
        Err(_) => return error(500, "decision_failed", None),
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
    // `authenticate` has already evaluated the checked session usability and
    // access transition. Row ownership is therefore the separate source
    // authorization decision over a currently active session.
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
    // A task update starts from the host's fixed idle transaction fact. The
    // checked source must admit that transition before this route constructs
    // a candidate state or reaches the durable snapshot commit.
    match host.decisions.update_is_committed(0) {
        Ok(true) => {}
        Ok(false) => return error(403, "update_not_admitted", Some(&committed.digest)),
        Err(_) => return error(500, "decision_failed", None),
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
    // A task deletion starts from the host's fixed idle transaction fact. The
    // checked source must admit that transition before the host constructs a
    // candidate state or reaches the durable snapshot commit.
    match host.decisions.delete_is_committed(0) {
        Ok(true) => {}
        Ok(false) => return error(403, "delete_not_admitted", Some(&committed.digest)),
        Err(_) => return error(500, "decision_failed", None),
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
    let outcome = match host.decisions.enqueue_outcome(
        existing.is_some(),
        existing.map(|job| job.desc.as_bytes()).unwrap_or(&[]),
        desc.as_bytes(),
    ) {
        Ok(value) if value <= 2 => value as u8,
        Ok(_) | Err(_) => return error(500, "decision_failed", None),
    };
    match outcome {
        0 => {
            if existing.is_some() {
                return error(500, "decision_failed", None);
            }
            // This route creates immediate Pending jobs only. Zero/zero are
            // explicit immediate-schedule facts, not a sampled wall clock;
            // scheduled jobs and clock authority are outside this route.
            match host
                .decisions
                .enqueue_is_legal(JobState::Pending.source_status(), 0, 0)
            {
                Ok(true) => {}
                Ok(false) => return error(403, "enqueue_not_admitted", Some(&committed.digest)),
                Err(_) => return error(500, "decision_failed", None),
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

/// Test-only crash injection exercised by
/// `reference_service_acceptance::completion_crash_after_delivery_before_commit_settles_uncertain_on_restart`.
///
/// If the named environment variable holds exactly this job's decimal id,
/// the whole process exits immediately -- after the durable webhook-delivery
/// attempt above has already settled and before the state commit below --
/// proving the crash-safety claim documented in
/// `docs/REFERENCE-SERVICE-HOST-V1.md` ("a crash between the two leaves a
/// pending job whose durable marker already exists, so the retry settles
/// `Uncertain` instead of redispatching") against a real killed and
/// restarted process, not only a delivery attempt that fails closed because
/// no peer exists. A bare environment variable never grants authority (see
/// `AGENTS.md`: capabilities stay host-granted, not ambient) and no real
/// deployment sets this one; debug-only so it never reaches a release
/// binary.
#[cfg(debug_assertions)]
fn crash_after_delivery_for_acceptance_test(job_id: i64) {
    const VAR: &str = "SEMAPRAX_REFERENCE_SERVICE_TEST_CRASH_AFTER_DELIVERY_JOB";
    if let Ok(value) = std::env::var(VAR) {
        if value.parse::<i64>() == Ok(job_id) {
            std::process::exit(91);
        }
    }
}

#[cfg(not(debug_assertions))]
fn crash_after_delivery_for_acceptance_test(_job_id: i64) {}

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
    match host
        .decisions
        .job_status_is_complete(job.state.source_status())
    {
        Ok(false) => {}
        Ok(true) => return error(409, "already_completed", Some(&committed.digest)),
        Err(_) => return error(500, "decision_failed", None),
    }
    let event_bytes = match delivery::completion_event_len(
        job.id,
        job.owner,
        &job.desc,
        host.secrets.webhook_key(),
    ) {
        Ok(length) => length,
        Err(_) => return error(500, "decision_failed", None),
    };
    let event_bytes = match u64::try_from(event_bytes) {
        Ok(event_bytes) => event_bytes,
        Err(_) => return error(500, "decision_failed", None),
    };
    match host.decisions.completed_job_export_is_admitted(
        0,
        1,
        event_bytes,
        host.telemetry_origin.as_bytes(),
    ) {
        Ok(true) => {}
        Ok(false) => return error(403, "export_not_admitted", Some(&committed.digest)),
        Err(_) => return error(500, "decision_failed", None),
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
    crash_after_delivery_for_acceptance_test(job.id);
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
#[path = "mapping/tests.rs"]
mod tests;
