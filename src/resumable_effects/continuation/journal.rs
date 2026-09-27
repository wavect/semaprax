//! Durable, append-only, authenticated journal for one continuation
//! invocation (`semaprax.resumable-journal.v1`).
//!
//! The store acquires no ambient authority. The caller opens and hands over
//! an owner-private directory ([`JournalDirectory`]); every file operation is
//! `openat` relative to that descriptor with `O_NOFOLLOW`. A journal is
//! created with `O_CREAT | O_EXCL`, every record is written with one
//! `O_APPEND` write and `fsync`ed before the append is acknowledged, and
//! records form an HMAC-authenticated hash chain under the caller's
//! [`SourceCheckpointKey`]. Recovery reads the observed tail exactly: a
//! complete line that fails its schema, chain or authentication is refused;
//! only a final newline-less fragment (an append that was never acknowledged)
//! may be cut, and only under the explicit [`TornTailPolicy`] that asks for it.

use super::ContinuationError;
use crate::interpreter::resumable::checkpoint::{
    channel_from_json, channel_json, scalar_from_json, scalar_json,
};
use crate::interpreter::resumable::ResumableChannelValue;
use crate::interpreter::ArgumentValue;
use crate::resumable_effects::source_checkpoint::SourceCheckpointKey;
use rustix::fs::{AtFlags, FileType, FlockOperation, Mode, OFlags};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::path::Path;

/// The aggregate, whole-function durable carrier.  It deliberately has no
/// shared record type or filename domain with the scalar v1 journal.
pub(super) mod channel;

/// Breaking changes to a journal line require a new schema identity.
pub const RESUMABLE_JOURNAL_SCHEMA_V1: &str = "semaprax.resumable-journal.v1";

const RECORD_DOMAIN: &[u8] = b"semaprax.resumable-journal-record.v1\0";
const NAME_DOMAIN: &[u8] = b"semaprax.resumable-journal-name.v1\0";
/// One Started, three records per suspension for the control profile's
/// sixteen suspensions (the sequential profile's eight fit inside), one
/// terminal and two settlement records.
pub(super) const MAX_RECORDS: usize =
    1 + 3 * crate::resumable_effects::lowering::control::MAX_CONTROL_SUSPENSIONS + 1 + 2;
pub(super) const MAX_JOURNAL_BYTES: usize = 512 * 1024;

/// What recovery does with a final fragment that has no terminating newline.
/// Such bytes were never acknowledged, because an append is acknowledged only
/// after its whole line, newline included, is written and `fsync`ed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TornTailPolicy {
    /// Refuse recovery with [`ContinuationError::TornTail`]; nothing changes.
    Refuse,
    /// Truncate exactly the bytes after the last newline, `fsync`, and
    /// continue from the last acknowledged record.
    TruncateUnacknowledged,
}

/// A caller-held capability for one owner-private journal directory. Holding
/// it is the only way the continuation driver can touch storage.
#[derive(Debug)]
pub struct JournalDirectory {
    fd: OwnedFd,
}

impl JournalDirectory {
    /// Open `path` without following a final symlink and validate it.
    pub fn open(path: &Path) -> Result<Self, ContinuationError> {
        let fd = rustix::fs::open(
            path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| ContinuationError::ForeignDirectory)?;
        Self::from_owned_fd(fd)
    }

    /// Adopt an already opened directory descriptor. It must be a directory
    /// owned by the effective user with no group or other permission bits.
    pub fn from_owned_fd(fd: OwnedFd) -> Result<Self, ContinuationError> {
        let stat = rustix::fs::fstat(&fd).map_err(|_| ContinuationError::ForeignDirectory)?;
        if !FileType::from_raw_mode(stat.st_mode).is_dir()
            || stat.st_uid != rustix::process::geteuid().as_raw()
            || stat.st_mode & 0o077 != 0
        {
            return Err(ContinuationError::ForeignDirectory);
        }
        Ok(Self { fd })
    }
}

/// One decoded journal record. Digests are raw SHA-256 bytes.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Record {
    Started {
        function: String,
        program_digest: [u8; 32],
        invocation_id: String,
        policy_epoch: u64,
        arguments_digest: [u8; 32],
        yield_count: u32,
        max_steps: u64,
    },
    Yielded {
        site: u32,
        envelope: String,
        envelope_digest: [u8; 32],
    },
    Dispatched {
        site: u32,
        envelope_digest: [u8; 32],
    },
    Answered {
        site: u32,
        envelope_digest: [u8; 32],
        answer: ResumableChannelValue,
        answer_digest: [u8; 32],
    },
    Completed {
        result: ArgumentValue,
    },
    Failed {
        class: String,
    },
    CleanupStarted,
    CleanupSettled {
        settlement: String,
    },
}

/// The open journal of one invocation: its descriptor, the next sequence
/// number and the digest of the last acknowledged line.
pub(super) struct Journal {
    file: File,
    next_seq: u64,
    prev: [u8; 32],
    #[cfg(test)]
    pub(super) fault: Option<super::tests::Fault>,
}

pub(super) fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

pub(super) fn hex(bytes: &[u8; 32]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(bytes))
}

fn unhex(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 || !text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return None;
    }
    let mut out = [0_u8; 32];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(out)
}

fn digest_field(value: &Value) -> Result<[u8; 32], ContinuationError> {
    value
        .as_str()
        .and_then(|text| text.strip_prefix("sha256:"))
        .and_then(unhex)
        .ok_or(ContinuationError::TamperedJournal)
}

/// The file name is a digest of the invocation identity, so no caller text
/// ever becomes a path component.
pub(super) fn journal_name(invocation_id: &str) -> String {
    let mut bytes = NAME_DOMAIN.to_vec();
    bytes.extend_from_slice(invocation_id.as_bytes());
    format!("{}.journal", hex(&sha256(&bytes)))
}

/// Issue #296 R20: widened to [`ResumableChannelValue`]. Byte-for-byte
/// identical to the pre-widening digest for a `Scalar` answer, since
/// [`channel_json`] renders that variant exactly like [`scalar_json`]
/// always did; only a genuinely aggregate answer ever produces a different
/// digest input.
pub(super) fn answer_digest(answer: &ResumableChannelValue) -> [u8; 32] {
    sha256(channel_json(answer).to_string().as_bytes())
}

fn record_json(record: &Record) -> Value {
    let digest = |bytes: &[u8; 32]| format!("sha256:{}", hex(bytes));
    match record {
        Record::Started {
            function,
            program_digest,
            invocation_id,
            policy_epoch,
            arguments_digest,
            yield_count,
            max_steps,
        } => json!({
            "kind": "started",
            "contract": super::RESUMABLE_CONTINUATION_CONTRACT_V1,
            "function": function,
            "program_digest": digest(program_digest),
            "invocation_id": invocation_id,
            "policy_epoch": policy_epoch,
            "arguments_digest": digest(arguments_digest),
            "yield_count": yield_count,
            "max_steps": max_steps,
        }),
        Record::Yielded {
            site,
            envelope,
            envelope_digest,
        } => json!({
            "kind": "yielded",
            "site": site,
            "envelope": envelope,
            "envelope_digest": digest(envelope_digest),
        }),
        Record::Dispatched {
            site,
            envelope_digest,
        } => json!({
            "kind": "dispatched",
            "site": site,
            "envelope_digest": digest(envelope_digest),
        }),
        Record::Answered {
            site,
            envelope_digest,
            answer,
            answer_digest,
        } => json!({
            "kind": "answered",
            "site": site,
            "envelope_digest": digest(envelope_digest),
            "answer": channel_json(answer),
            "answer_digest": digest(answer_digest),
        }),
        Record::Completed { result } => json!({"kind": "completed", "result": scalar_json(result)}),
        Record::Failed { class } => json!({"kind": "failed", "class": class}),
        Record::CleanupStarted => json!({"kind": "cleanup_started"}),
        Record::CleanupSettled { settlement } => {
            json!({"kind": "cleanup_settled", "settlement": settlement})
        }
    }
}

fn exact_keys(value: &Value, expected: &[&str]) -> Result<(), ContinuationError> {
    let object = value
        .as_object()
        .ok_or(ContinuationError::TamperedJournal)?;
    if object.len() != expected.len() || !expected.iter().all(|key| object.contains_key(*key)) {
        return Err(ContinuationError::TamperedJournal);
    }
    Ok(())
}

fn text(value: &Value, field: &str) -> Result<String, ContinuationError> {
    value[field]
        .as_str()
        .map(str::to_owned)
        .ok_or(ContinuationError::TamperedJournal)
}

fn site(value: &Value) -> Result<u32, ContinuationError> {
    value["site"]
        .as_u64()
        .and_then(|site| u32::try_from(site).ok())
        .ok_or(ContinuationError::TamperedJournal)
}

fn scalar(value: &Value) -> Result<ArgumentValue, ContinuationError> {
    scalar_from_json(value).map_err(|_| ContinuationError::TamperedJournal)
}

fn channel(value: &Value) -> Result<ResumableChannelValue, ContinuationError> {
    channel_from_json(value).map_err(|_| ContinuationError::TamperedJournal)
}

fn parse_record(value: &Value) -> Result<Record, ContinuationError> {
    let kind = value["kind"]
        .as_str()
        .ok_or(ContinuationError::TamperedJournal)?;
    let record = match kind {
        "started" => {
            exact_keys(
                value,
                &[
                    "kind",
                    "contract",
                    "function",
                    "program_digest",
                    "invocation_id",
                    "policy_epoch",
                    "arguments_digest",
                    "yield_count",
                    "max_steps",
                ],
            )?;
            if value["contract"] != super::RESUMABLE_CONTINUATION_CONTRACT_V1 {
                return Err(ContinuationError::SchemaMismatch);
            }
            Record::Started {
                function: text(value, "function")?,
                program_digest: digest_field(&value["program_digest"])?,
                invocation_id: text(value, "invocation_id")?,
                policy_epoch: value["policy_epoch"]
                    .as_u64()
                    .ok_or(ContinuationError::TamperedJournal)?,
                arguments_digest: digest_field(&value["arguments_digest"])?,
                yield_count: value["yield_count"]
                    .as_u64()
                    .and_then(|count| u32::try_from(count).ok())
                    .ok_or(ContinuationError::TamperedJournal)?,
                max_steps: value["max_steps"]
                    .as_u64()
                    .ok_or(ContinuationError::TamperedJournal)?,
            }
        }
        "yielded" => {
            exact_keys(value, &["kind", "site", "envelope", "envelope_digest"])?;
            Record::Yielded {
                site: site(value)?,
                envelope: text(value, "envelope")?,
                envelope_digest: digest_field(&value["envelope_digest"])?,
            }
        }
        "dispatched" => {
            exact_keys(value, &["kind", "site", "envelope_digest"])?;
            Record::Dispatched {
                site: site(value)?,
                envelope_digest: digest_field(&value["envelope_digest"])?,
            }
        }
        "answered" => {
            exact_keys(
                value,
                &["kind", "site", "envelope_digest", "answer", "answer_digest"],
            )?;
            Record::Answered {
                site: site(value)?,
                envelope_digest: digest_field(&value["envelope_digest"])?,
                answer: channel(&value["answer"])?,
                answer_digest: digest_field(&value["answer_digest"])?,
            }
        }
        "completed" => {
            exact_keys(value, &["kind", "result"])?;
            Record::Completed {
                result: scalar(&value["result"])?,
            }
        }
        "failed" => {
            exact_keys(value, &["kind", "class"])?;
            Record::Failed {
                class: text(value, "class")?,
            }
        }
        "cleanup_started" => {
            exact_keys(value, &["kind"])?;
            Record::CleanupStarted
        }
        "cleanup_settled" => {
            exact_keys(value, &["kind", "settlement"])?;
            Record::CleanupSettled {
                settlement: text(value, "settlement")?,
            }
        }
        _ => return Err(ContinuationError::TamperedJournal),
    };
    // Every field is re-rendered: a record is accepted only in its one
    // canonical projection.
    if record_json(&record) != *value {
        return Err(ContinuationError::TamperedJournal);
    }
    Ok(record)
}

fn unsigned(seq: u64, prev: &[u8; 32], record: &Record) -> Value {
    json!({
        "schema": RESUMABLE_JOURNAL_SCHEMA_V1,
        "seq": seq,
        "prev": format!("sha256:{}", hex(prev)),
        "record": record_json(record),
    })
}

fn render_line(key: &SourceCheckpointKey, seq: u64, prev: &[u8; 32], record: &Record) -> String {
    let payload = unsigned(seq, prev, record);
    let tag = key.authenticate(RECORD_DOMAIN, payload.to_string().as_bytes());
    json!({
        "schema": RESUMABLE_JOURNAL_SCHEMA_V1,
        "seq": seq,
        "prev": format!("sha256:{}", hex(prev)),
        "record": payload["record"].clone(),
        "mac": format!("hmac-sha256:{}", hex(&tag)),
    })
    .to_string()
}

fn io(_: impl Sized) -> ContinuationError {
    ContinuationError::Storage
}

fn validate_file(file: &File) -> Result<(), ContinuationError> {
    let stat = rustix::fs::fstat(file.as_fd()).map_err(io)?;
    if !FileType::from_raw_mode(stat.st_mode).is_file()
        || stat.st_uid != rustix::process::geteuid().as_raw()
        || stat.st_mode & 0o077 != 0
        || stat.st_nlink != 1
    {
        return Err(ContinuationError::ForeignDirectory);
    }
    Ok(())
}

/// A `flock` belongs to the open file description, not to the `File`. When
/// another thread of this process forks (for example `std::process::Command`
/// in a concurrent caller) between this journal's `open` and its drop, the
/// child briefly holds a duplicate of the descriptor until its `exec` closes
/// it (`O_CLOEXEC`), so the lock outlives the dropped `File` by that window.
/// A reopen on the same path during it sees `WOULDBLOCK`; this was observed
/// under heavy parallel test load (issue #296). The bounded retry absorbs
/// that window only: a genuinely live writer keeps the lock far longer than
/// `LOCK_RETRY_ATTEMPTS * LOCK_RETRY_DELAY` and is still refused.
const LOCK_RETRY_ATTEMPTS: u32 = 20;
const LOCK_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(1);

/// Hold an exclusive advisory lock for the descriptor's lifetime. Another
/// open description (another process, or another live instance in this one)
/// is refused rather than waited for, past `LOCK_RETRY_ATTEMPTS`'s bounded,
/// millisecond-scale retry window (see its own doc comment for why that
/// window exists at all).
fn lock_exclusive(file: &File) -> Result<(), ContinuationError> {
    for attempt in 0..LOCK_RETRY_ATTEMPTS {
        match rustix::fs::flock(file.as_fd(), FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => return Ok(()),
            Err(rustix::io::Errno::WOULDBLOCK) if attempt + 1 < LOCK_RETRY_ATTEMPTS => {
                std::thread::sleep(LOCK_RETRY_DELAY);
            }
            Err(rustix::io::Errno::WOULDBLOCK) => return Err(ContinuationError::JournalBusy),
            Err(_) => return Err(ContinuationError::Storage),
        }
    }
    unreachable!("LOCK_RETRY_ATTEMPTS is a nonzero constant, so the loop always returns above")
}

/// Make a new directory entry durable. On Apple platforms `fsync` does not
/// flush the drive cache, so `F_FULLFSYNC` is tried first, as `sync_all` does
/// for files.
fn sync_directory(directory: &JournalDirectory) -> Result<(), ContinuationError> {
    #[cfg(target_vendor = "apple")]
    if rustix::fs::fcntl_fullfsync(&directory.fd).is_ok() {
        return Ok(());
    }
    rustix::fs::fsync(&directory.fd).map_err(io)
}

impl Journal {
    /// Create a brand-new journal. An existing file of the same name is a
    /// refusal, never an overwrite. If validation, locking, or the directory
    /// sync then fails, the fresh entry is unlinked again, so a failed create
    /// never blocks a later start with `AlreadyStarted`.
    pub(super) fn create(
        directory: &JournalDirectory,
        invocation_id: &str,
    ) -> Result<Self, ContinuationError> {
        let name = journal_name(invocation_id);
        let fd = rustix::fs::openat(
            &directory.fd,
            name.as_str(),
            OFlags::RDWR
                | OFlags::APPEND
                | OFlags::CREATE
                | OFlags::EXCL
                | OFlags::NOFOLLOW
                | OFlags::CLOEXEC,
            Mode::from_bits_truncate(0o600),
        )
        .map_err(|error| {
            if error == rustix::io::Errno::EXIST {
                ContinuationError::AlreadyStarted
            } else {
                ContinuationError::Storage
            }
        })?;
        let file = File::from(fd);
        // The entry is ours alone (`O_EXCL` just created it): on any failure
        // below, remove it again. Only a crash between creation and the first
        // append can still strand an empty file, and recovery treats an empty
        // journal as never started. On success the new directory entry is
        // durable before the first record.
        if let Err(error) = validate_file(&file)
            .and_then(|()| lock_exclusive(&file))
            .and_then(|()| sync_directory(directory))
        {
            let _ = rustix::fs::unlinkat(&directory.fd, name.as_str(), AtFlags::empty());
            return Err(error);
        }
        Ok(Self {
            file,
            next_seq: 0,
            prev: [0; 32],
            #[cfg(test)]
            fault: super::tests::armed_fault(),
        })
    }

    /// Open an existing journal and return its verified records.
    pub(super) fn open(
        directory: &JournalDirectory,
        invocation_id: &str,
        key: &SourceCheckpointKey,
        torn: TornTailPolicy,
    ) -> Result<(Self, Vec<Record>), ContinuationError> {
        let fd = rustix::fs::openat(
            &directory.fd,
            journal_name(invocation_id),
            OFlags::RDWR | OFlags::APPEND | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| match error {
            rustix::io::Errno::NOENT => ContinuationError::NotStarted,
            rustix::io::Errno::LOOP => ContinuationError::ForeignDirectory,
            _ => ContinuationError::Storage,
        })?;
        let mut file = File::from(fd);
        validate_file(&file)?;
        // The single-writer lock is taken before anything is read, so no
        // other live instance can append or truncate under this recovery.
        lock_exclusive(&file)?;
        let mut bytes = Vec::new();
        (&mut file)
            .take(MAX_JOURNAL_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(io)?;
        if bytes.len() > MAX_JOURNAL_BYTES {
            return Err(ContinuationError::TamperedJournal);
        }
        let acknowledged = bytes
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |index| index + 1);
        let mut journal = Self {
            file,
            next_seq: 0,
            prev: [0; 32],
            #[cfg(test)]
            fault: super::tests::armed_fault(),
        };
        let mut records = Vec::new();
        for chunk in bytes[..acknowledged].split_inclusive(|byte| *byte == b'\n') {
            let line = &chunk[..chunk.len() - 1];
            if line.is_empty() || records.len() == MAX_RECORDS {
                return Err(ContinuationError::TamperedJournal);
            }
            records.push(journal.verify_line(key, line)?);
        }
        // Only after every acknowledged record verified is the torn fragment
        // considered, so truncation can never hide a tampered record.
        if acknowledged != bytes.len() {
            match torn {
                TornTailPolicy::Refuse => return Err(ContinuationError::TornTail),
                TornTailPolicy::TruncateUnacknowledged => {
                    rustix::fs::ftruncate(journal.file.as_fd(), acknowledged as u64).map_err(io)?;
                    journal.file.sync_all().map_err(io)?;
                }
            }
        }
        Ok((journal, records))
    }

    fn verify_line(
        &mut self,
        key: &SourceCheckpointKey,
        line: &[u8],
    ) -> Result<Record, ContinuationError> {
        let document: Value =
            serde_json::from_slice(line).map_err(|_| ContinuationError::TamperedJournal)?;
        exact_keys(&document, &["schema", "seq", "prev", "record", "mac"])?;
        if document["schema"] != RESUMABLE_JOURNAL_SCHEMA_V1 {
            return Err(ContinuationError::SchemaMismatch);
        }
        if document["seq"].as_u64() != Some(self.next_seq)
            || digest_field(&document["prev"])? != self.prev
        {
            return Err(ContinuationError::TamperedJournal);
        }
        let record = parse_record(&document["record"])?;
        let tag = document["mac"]
            .as_str()
            .and_then(|mac| mac.strip_prefix("hmac-sha256:"))
            .and_then(unhex)
            .ok_or(ContinuationError::TamperedJournal)?;
        let payload = unsigned(self.next_seq, &self.prev, &record);
        if !key.verify(RECORD_DOMAIN, payload.to_string().as_bytes(), &tag)
            || render_line(key, self.next_seq, &self.prev, &record).as_bytes() != line
        {
            return Err(ContinuationError::TamperedJournal);
        }
        self.next_seq += 1;
        self.prev = sha256(line);
        Ok(record)
    }

    /// Append one record with a single write and `fsync` it. Returning `Ok`
    /// is the acknowledgement; nothing after an `Err` may assume the record.
    /// Appends past `MAX_RECORDS` records or `MAX_JOURNAL_BYTES` bytes are
    /// refused before any byte is written, matching the read ceilings.
    pub(super) fn append(
        &mut self,
        key: &SourceCheckpointKey,
        record: &Record,
    ) -> Result<(), ContinuationError> {
        if self.next_seq as usize >= MAX_RECORDS {
            return Err(ContinuationError::TamperedJournal);
        }
        #[cfg(test)]
        let after = match self.fault.as_mut().map(|fault| fault.before_append()) {
            Some(super::tests::FaultAction::CrashBefore) => return Err(ContinuationError::Storage),
            Some(super::tests::FaultAction::CrashAfter) => true,
            _ => false,
        };
        let line = render_line(key, self.next_seq, &self.prev, record);
        let mut bytes = line.clone().into_bytes();
        bytes.push(b'\n');
        let size = rustix::fs::fstat(self.file.as_fd()).map_err(io)?.st_size;
        let end = u64::try_from(size).map_err(io)? + bytes.len() as u64;
        if end > MAX_JOURNAL_BYTES as u64 {
            return Err(ContinuationError::TamperedJournal);
        }
        self.file.write_all(&bytes).map_err(io)?;
        self.file.sync_all().map_err(io)?;
        self.next_seq += 1;
        self.prev = sha256(line.as_bytes());
        #[cfg(test)]
        if after {
            return Err(ContinuationError::Storage);
        }
        Ok(())
    }
}
