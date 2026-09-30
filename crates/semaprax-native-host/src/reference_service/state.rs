//! Canonical reference-service state snapshots.
//!
//! One snapshot is the complete durable state: accounts, sessions, tasks,
//! and jobs. Snapshots are content-addressed through the existing durable
//! checkpoint store ([`OutboundDeliveryStore`](crate::outbound_delivery_store::OutboundDeliveryStore)
//! with [`ServiceState`](crate::outbound_delivery_store::OutboundCheckpointKind::ServiceState)),
//! so the store's discipline applies unchanged: atomic create-new commits,
//! exact-digest loads, and no enumeration or "latest" selection. The host
//! operator retains each accepted digest (every mutating response carries
//! the new one); a digest the operator lost is unrecoverable by design.

use super::json::{self, JsonRefusal, JsonValue};

pub const STATE_SCHEMA: &str = "semaprax.reference-service.state.v3";
pub const MAX_STATE_BYTES: usize = 192 * 1024;

/// The persisted account inventory limit supplied to the checked registration
/// admission decision before the password host performs any work.
pub(crate) const MAX_ACCOUNTS: usize = 64;
const MAX_SESSIONS: usize = 256;
const MAX_TASKS: usize = 256;
const MAX_JOBS: usize = 256;
const MAX_NAME_BYTES: usize = 64;
const MAX_TITLE_BYTES: usize = 256;
const MAX_KEY_BYTES: usize = 128;
const MAX_DESC_BYTES: usize = 256;
const MAX_PHC_BYTES: usize = 512;
const SESSION_ID_BYTES: usize = 16;
const SESSION_ID_HEX: usize = SESSION_ID_BYTES * 2;
const MAX_EVIDENCE_DIGEST_BYTES: usize = 128;

/// Stable refusal categories for a snapshot that is not exactly valid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateRefusal {
    TooLarge,
    Malformed,
    Closed,
    OutOfBounds,
    UnknownReference,
    DuplicateId,
}

impl From<JsonRefusal> for StateRefusal {
    fn from(refusal: JsonRefusal) -> Self {
        match refusal {
            JsonRefusal::TooLarge | JsonRefusal::TooManyValues => Self::TooLarge,
            _ => Self::Malformed,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskStatus {
    Open,
    Done,
}

impl TaskStatus {
    fn decode(value: &str) -> Option<Self> {
        match value {
            "open" => Some(Self::Open),
            "done" => Some(Self::Done),
            _ => None,
        }
    }

    fn encode(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Done => "done",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobState {
    Pending,
    Completed,
}

impl JobState {
    /// The checked scaffold's durable-job state code for this persisted
    /// reference-service state. The host owns this representation mapping;
    /// checked source selects whether the mapped code is terminal.
    pub(crate) fn source_status(self) -> u64 {
        match self {
            Self::Pending => 0,
            Self::Completed => 4,
        }
    }

    fn decode(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "completed" => Some(Self::Completed),
            _ => None,
        }
    }

    fn encode(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Completed => "completed",
        }
    }
}

/// The settled outbound-delivery outcome for one completed job. `None` means
/// no delivery was ever attempted; every other variant is terminal and is
/// never retried, re-attempted, or cleared.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WebhookSettlement {
    None,
    Delivered { evidence_digest: String },
    Failed,
    Uncertain,
}

impl WebhookSettlement {
    fn decode(value: &JsonValue) -> Result<Self, StateRefusal> {
        let text = value.as_str().ok_or(StateRefusal::Malformed)?;
        if text == "none" {
            return Ok(Self::None);
        }
        if text == "failed" {
            return Ok(Self::Failed);
        }
        if text == "uncertain" {
            return Ok(Self::Uncertain);
        }
        match text.strip_prefix("delivered:") {
            Some(digest)
                if !digest.is_empty()
                    && digest.len() <= MAX_EVIDENCE_DIGEST_BYTES
                    && digest.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-')
                    }) =>
            {
                Ok(Self::Delivered {
                    evidence_digest: digest.to_owned(),
                })
            }
            _ => Err(StateRefusal::Malformed),
        }
    }

    fn encode(&self) -> String {
        match self {
            Self::None => "none".to_owned(),
            Self::Failed => "failed".to_owned(),
            Self::Uncertain => "uncertain".to_owned(),
            Self::Delivered { evidence_digest } => format!("delivered:{evidence_digest}"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub password_phc: String,
    pub created_seq: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Session {
    pub id: String,
    pub account: i64,
    /// The checked `std.auth.session` state code. Only zero is active;
    /// nonzero terminal values remain terminal across restart.
    pub state: u8,
    pub idle_deadline_tick: i64,
    pub absolute_deadline_tick: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Task {
    pub id: i64,
    pub owner: i64,
    pub title: String,
    pub status: TaskStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Job {
    pub id: i64,
    pub owner: i64,
    pub key: String,
    pub desc: String,
    pub state: JobState,
    pub webhook: WebhookSettlement,
}

/// One complete durable snapshot. `seq` counts accepted mutations from the
/// empty genesis (`seq == 0`); every committed mutation advances it by one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceState {
    pub seq: i64,
    pub accounts: Vec<Account>,
    pub sessions: Vec<Session>,
    pub tasks: Vec<Task>,
    pub jobs: Vec<Job>,
}

impl ServiceState {
    /// The empty genesis snapshot. Its digest is stable and may be
    /// recomputed by any holder of this source.
    pub fn empty() -> Self {
        Self {
            seq: 0,
            accounts: Vec::new(),
            sessions: Vec::new(),
            tasks: Vec::new(),
            jobs: Vec::new(),
        }
    }

    /// Decode one exact snapshot. Anything outside the closed schema,
    /// bounds, uniqueness, or reference rules is refused.
    pub fn decode(bytes: &[u8]) -> Result<Self, StateRefusal> {
        if bytes.len() > MAX_STATE_BYTES {
            return Err(StateRefusal::TooLarge);
        }
        let value = json::parse(bytes, MAX_STATE_BYTES)?;
        let root = value
            .closed(&["accounts", "jobs", "schema", "seq", "sessions", "tasks"])
            .ok_or(StateRefusal::Closed)?;
        let schema = member_str(root, "schema")?;
        if schema != STATE_SCHEMA {
            return Err(StateRefusal::Malformed);
        }
        let seq = member_i64(root, "seq")?;
        if seq < 0 {
            return Err(StateRefusal::OutOfBounds);
        }
        let accounts = decode_accounts(member(root, "accounts")?)?;
        let sessions = decode_sessions(member(root, "sessions")?, &accounts)?;
        let tasks = decode_tasks(member(root, "tasks")?, &accounts)?;
        let jobs = decode_jobs(member(root, "jobs")?, &accounts)?;
        Ok(Self {
            seq,
            accounts,
            sessions,
            tasks,
            jobs,
        })
    }

    /// Render the canonical bytes this digest addresses.
    pub fn render(&self) -> String {
        let accounts = self
            .accounts
            .iter()
            .map(|account| {
                JsonValue::Object(vec![
                    (
                        "created_seq".to_owned(),
                        JsonValue::Int(account.created_seq),
                    ),
                    ("id".to_owned(), JsonValue::Int(account.id)),
                    ("name".to_owned(), JsonValue::Str(account.name.clone())),
                    (
                        "password_phc".to_owned(),
                        JsonValue::Str(account.password_phc.clone()),
                    ),
                ])
            })
            .collect();
        let sessions = self
            .sessions
            .iter()
            .map(|session| {
                JsonValue::Object(vec![
                    ("account".to_owned(), JsonValue::Int(session.account)),
                    (
                        "absolute_deadline_tick".to_owned(),
                        JsonValue::Int(session.absolute_deadline_tick),
                    ),
                    ("id".to_owned(), JsonValue::Str(session.id.clone())),
                    (
                        "idle_deadline_tick".to_owned(),
                        JsonValue::Int(session.idle_deadline_tick),
                    ),
                    ("state".to_owned(), JsonValue::Int(i64::from(session.state))),
                ])
            })
            .collect();
        let tasks = self
            .tasks
            .iter()
            .map(|task| {
                JsonValue::Object(vec![
                    ("id".to_owned(), JsonValue::Int(task.id)),
                    ("owner".to_owned(), JsonValue::Int(task.owner)),
                    (
                        "status".to_owned(),
                        JsonValue::Str(task.status.encode().to_owned()),
                    ),
                    ("title".to_owned(), JsonValue::Str(task.title.clone())),
                ])
            })
            .collect();
        let jobs = self
            .jobs
            .iter()
            .map(|job| {
                JsonValue::Object(vec![
                    ("desc".to_owned(), JsonValue::Str(job.desc.clone())),
                    ("id".to_owned(), JsonValue::Int(job.id)),
                    ("key".to_owned(), JsonValue::Str(job.key.clone())),
                    ("owner".to_owned(), JsonValue::Int(job.owner)),
                    (
                        "state".to_owned(),
                        JsonValue::Str(job.state.encode().to_owned()),
                    ),
                    ("webhook".to_owned(), JsonValue::Str(job.webhook.encode())),
                ])
            })
            .collect();
        json::render(&JsonValue::Object(vec![
            ("accounts".to_owned(), JsonValue::Array(accounts)),
            ("jobs".to_owned(), JsonValue::Array(jobs)),
            ("schema".to_owned(), JsonValue::Str(STATE_SCHEMA.to_owned())),
            ("seq".to_owned(), JsonValue::Int(self.seq)),
            ("sessions".to_owned(), JsonValue::Array(sessions)),
            ("tasks".to_owned(), JsonValue::Array(tasks)),
        ]))
    }

    /// The content digest addressing `rendered`: `sha256:` plus lowercase
    /// hex over the exact bytes.
    pub fn digest(rendered: &str) -> String {
        super::content_digest(rendered.as_bytes())
    }

    pub fn account_by_name(&self, name: &str) -> Option<&Account> {
        self.accounts.iter().find(|account| account.name == name)
    }

    pub fn account_by_id(&self, id: i64) -> Option<&Account> {
        self.accounts.iter().find(|account| account.id == id)
    }

    pub fn session_by_id(&self, id: &str) -> Option<&Session> {
        self.sessions.iter().find(|session| session.id == id)
    }

    pub fn task_by_id(&self, id: i64) -> Option<&Task> {
        self.tasks.iter().find(|task| task.id == id)
    }

    pub fn job_by_id(&self, id: i64) -> Option<&Job> {
        self.jobs.iter().find(|job| job.id == id)
    }

    pub fn job_by_key(&self, key: &str) -> Option<&Job> {
        self.jobs.iter().find(|job| job.key == key)
    }
}

fn member<'a>(root: &'a [(String, JsonValue)], key: &str) -> Result<&'a JsonValue, StateRefusal> {
    root.iter()
        .find(|(candidate, _)| candidate == key)
        .map(|(_, value)| value)
        .ok_or(StateRefusal::Closed)
}

fn member_str<'a>(root: &'a [(String, JsonValue)], key: &str) -> Result<&'a str, StateRefusal> {
    member(root, key)?.as_str().ok_or(StateRefusal::Malformed)
}

fn member_i64(root: &[(String, JsonValue)], key: &str) -> Result<i64, StateRefusal> {
    member(root, key)?.as_i64().ok_or(StateRefusal::Malformed)
}

fn bounded_text<'a>(
    root: &'a [(String, JsonValue)],
    key: &str,
    max_bytes: usize,
) -> Result<&'a str, StateRefusal> {
    let text = member_str(root, key)?;
    if text.is_empty() || text.len() > max_bytes {
        return Err(StateRefusal::OutOfBounds);
    }
    Ok(text)
}

fn decode_accounts(root: &JsonValue) -> Result<Vec<Account>, StateRefusal> {
    let values = root.as_array().ok_or(StateRefusal::Malformed)?;
    if values.len() > MAX_ACCOUNTS {
        return Err(StateRefusal::OutOfBounds);
    }
    let mut accounts = Vec::with_capacity(values.len());
    for value in values {
        let row = value
            .closed(&["created_seq", "id", "name", "password_phc"])
            .ok_or(StateRefusal::Closed)?;
        let id = member_i64(row, "id")?;
        let created_seq = member_i64(row, "created_seq")?;
        if id <= 0 || created_seq <= 0 {
            return Err(StateRefusal::OutOfBounds);
        }
        let name = bounded_text(row, "name", MAX_NAME_BYTES)?;
        let password_phc = bounded_text(row, "password_phc", MAX_PHC_BYTES)?;
        if accounts.iter().any(|account: &Account| account.id == id)
            || accounts
                .iter()
                .any(|account: &Account| account.name == name)
        {
            return Err(StateRefusal::DuplicateId);
        }
        accounts.push(Account {
            id,
            name: name.to_owned(),
            password_phc: password_phc.to_owned(),
            created_seq,
        });
    }
    Ok(accounts)
}

fn valid_session_id(text: &str) -> bool {
    text.len() == SESSION_ID_HEX
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn decode_sessions(root: &JsonValue, accounts: &[Account]) -> Result<Vec<Session>, StateRefusal> {
    let values = root.as_array().ok_or(StateRefusal::Malformed)?;
    if values.len() > MAX_SESSIONS {
        return Err(StateRefusal::OutOfBounds);
    }
    let mut sessions = Vec::with_capacity(values.len());
    for value in values {
        let row = value
            .closed(&[
                "absolute_deadline_tick",
                "account",
                "id",
                "idle_deadline_tick",
                "state",
            ])
            .ok_or(StateRefusal::Closed)?;
        let id = member_str(row, "id")?;
        if !valid_session_id(id) {
            return Err(StateRefusal::OutOfBounds);
        }
        let account = member_i64(row, "account")?;
        let idle_deadline_tick = member_i64(row, "idle_deadline_tick")?;
        let absolute_deadline_tick = member_i64(row, "absolute_deadline_tick")?;
        let state =
            u8::try_from(member_i64(row, "state")?).map_err(|_| StateRefusal::OutOfBounds)?;
        if state > 5 {
            return Err(StateRefusal::OutOfBounds);
        }
        if idle_deadline_tick < 0
            || absolute_deadline_tick < 0
            || idle_deadline_tick > absolute_deadline_tick
            || !accounts.iter().any(|candidate| candidate.id == account)
        {
            return Err(StateRefusal::UnknownReference);
        }
        if sessions.iter().any(|session: &Session| session.id == id) {
            return Err(StateRefusal::DuplicateId);
        }
        sessions.push(Session {
            id: id.to_owned(),
            account,
            state,
            idle_deadline_tick,
            absolute_deadline_tick,
        });
    }
    Ok(sessions)
}

fn decode_tasks(root: &JsonValue, accounts: &[Account]) -> Result<Vec<Task>, StateRefusal> {
    let values = root.as_array().ok_or(StateRefusal::Malformed)?;
    if values.len() > MAX_TASKS {
        return Err(StateRefusal::OutOfBounds);
    }
    let mut tasks = Vec::with_capacity(values.len());
    for value in values {
        let row = value
            .closed(&["id", "owner", "status", "title"])
            .ok_or(StateRefusal::Closed)?;
        let id = member_i64(row, "id")?;
        if id <= 0 || tasks.iter().any(|task: &Task| task.id == id) {
            return Err(if id <= 0 {
                StateRefusal::OutOfBounds
            } else {
                StateRefusal::DuplicateId
            });
        }
        let owner = member_i64(row, "owner")?;
        if !accounts.iter().any(|candidate| candidate.id == owner) {
            return Err(StateRefusal::UnknownReference);
        }
        let status =
            TaskStatus::decode(member_str(row, "status")?).ok_or(StateRefusal::Malformed)?;
        let title = bounded_text(row, "title", MAX_TITLE_BYTES)?;
        tasks.push(Task {
            id,
            owner,
            title: title.to_owned(),
            status,
        });
    }
    Ok(tasks)
}

fn decode_jobs(root: &JsonValue, accounts: &[Account]) -> Result<Vec<Job>, StateRefusal> {
    let values = root.as_array().ok_or(StateRefusal::Malformed)?;
    if values.len() > MAX_JOBS {
        return Err(StateRefusal::OutOfBounds);
    }
    let mut jobs = Vec::with_capacity(values.len());
    for value in values {
        let row = value
            .closed(&["desc", "id", "key", "owner", "state", "webhook"])
            .ok_or(StateRefusal::Closed)?;
        let id = member_i64(row, "id")?;
        if id <= 0 || jobs.iter().any(|job: &Job| job.id == id) {
            return Err(if id <= 0 {
                StateRefusal::OutOfBounds
            } else {
                StateRefusal::DuplicateId
            });
        }
        let owner = member_i64(row, "owner")?;
        if !accounts.iter().any(|candidate| candidate.id == owner) {
            return Err(StateRefusal::UnknownReference);
        }
        let key = bounded_text(row, "key", MAX_KEY_BYTES)?;
        if jobs.iter().any(|job: &Job| job.key == key) {
            return Err(StateRefusal::DuplicateId);
        }
        let desc = bounded_text(row, "desc", MAX_DESC_BYTES)?;
        let state = JobState::decode(member_str(row, "state")?).ok_or(StateRefusal::Malformed)?;
        let webhook = WebhookSettlement::decode(member(row, "webhook")?)?;
        if state == JobState::Pending && webhook != WebhookSettlement::None {
            return Err(StateRefusal::Malformed);
        }
        jobs.push(Job {
            id,
            owner,
            key: key.to_owned(),
            desc: desc.to_owned(),
            state,
            webhook,
        });
    }
    Ok(jobs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn populated() -> ServiceState {
        ServiceState {
            seq: 3,
            accounts: vec![Account {
                id: 1,
                name: "alice".to_owned(),
                password_phc: "$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$hash".to_owned(),
                created_seq: 1,
            }],
            sessions: vec![Session {
                id: "0123456789abcdef0123456789abcdef".to_owned(),
                account: 1,
                state: 0,
                idle_deadline_tick: 100,
                absolute_deadline_tick: 200,
            }],
            tasks: vec![Task {
                id: 1,
                owner: 1,
                title: "write the report".to_owned(),
                status: TaskStatus::Open,
            }],
            jobs: vec![Job {
                id: 1,
                owner: 1,
                key: "job-1".to_owned(),
                desc: "task-1".to_owned(),
                state: JobState::Completed,
                webhook: WebhookSettlement::Delivered {
                    evidence_digest: "sha256:abc".to_owned(),
                },
            }],
        }
    }

    #[test]
    fn genesis_and_populated_round_trip_with_stable_digest() {
        let genesis = ServiceState::empty();
        let rendered = genesis.render();
        assert_eq!(
            rendered,
            r#"{"accounts":[],"jobs":[],"schema":"semaprax.reference-service.state.v3","seq":0,"sessions":[],"tasks":[]}"#
        );
        assert_eq!(ServiceState::decode(rendered.as_bytes()).unwrap(), genesis);
        let first = ServiceState::digest(&rendered);
        assert!(first.starts_with("sha256:"));
        assert_eq!(first.len(), 7 + 64);

        let state = populated();
        let rendered = state.render();
        assert_eq!(ServiceState::decode(rendered.as_bytes()).unwrap(), state);
        assert_eq!(ServiceState::digest(&rendered).len(), 7 + 64);
        // Canonical bytes are stable: rendering twice yields identical bytes.
        assert_eq!(state.render(), rendered);
    }

    #[test]
    fn hostile_snapshots_refuse() {
        // Unknown schema.
        let mut tampered = populated().render().replace(STATE_SCHEMA, "other");
        assert_eq!(
            ServiceState::decode(tampered.as_bytes()),
            Err(StateRefusal::Malformed)
        );
        // Extra member breaks the closed root.
        tampered = populated()
            .render()
            .replace("\"seq\":3", "\"seq\":3,\"zzz\":1");
        assert_eq!(
            ServiceState::decode(tampered.as_bytes()),
            Err(StateRefusal::Closed)
        );
        // Dangling owner reference.
        tampered = populated()
            .render()
            .replace("\"owner\":1", "\"owner\":99")
            .replacen("\"owner\":99", "\"owner\":1", 1);
        assert_eq!(
            ServiceState::decode(tampered.as_bytes()),
            Err(StateRefusal::UnknownReference)
        );
        // Duplicate task id.
        let mut state = populated();
        state.tasks.push(Task {
            id: 1,
            owner: 1,
            title: "dupe".to_owned(),
            status: TaskStatus::Open,
        });
        assert_eq!(
            ServiceState::decode(state.render().as_bytes()),
            Err(StateRefusal::DuplicateId)
        );
        // Unknown source state codes cannot enter a durable snapshot.
        let mut invalid_session_state = populated();
        invalid_session_state.sessions[0].state = 6;
        assert_eq!(
            ServiceState::decode(invalid_session_state.render().as_bytes()),
            Err(StateRefusal::OutOfBounds)
        );
        // Pending job with a settled webhook is incoherent.
        let mut pending = populated();
        pending.jobs[0].state = JobState::Pending;
        assert_eq!(
            ServiceState::decode(pending.render().as_bytes()),
            Err(StateRefusal::Malformed)
        );
        // Over-bound title.
        let mut big = populated();
        big.tasks[0].title = "t".repeat(MAX_TITLE_BYTES + 1);
        assert_eq!(
            ServiceState::decode(big.render().as_bytes()),
            Err(StateRefusal::OutOfBounds)
        );
        // Truncated bytes.
        let rendered = populated().render();
        assert_eq!(
            ServiceState::decode(&rendered.as_bytes()[..rendered.len() / 2]),
            Err(StateRefusal::Malformed)
        );
    }
}
