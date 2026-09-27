//! Versioned aggregate whole-function journal carrier.
//!
//! This is deliberately separate from the scalar v1 journal: its path name,
//! record authentication domain, schema, Started carrier and Completed value
//! all reject scalar recovery before any driver can reinterpret aggregate data.

use super::{
    channel, digest_field, exact_keys, hex, io, lock_exclusive, sha256, sync_directory, text,
    unhex, validate_file, JournalDirectory, TornTailPolicy, MAX_JOURNAL_BYTES, MAX_RECORDS,
};
use crate::interpreter::resumable::checkpoint::channel_json;
use crate::interpreter::resumable::ResumableChannelValue;
use crate::resumable_effects::continuation::ContinuationError;
use crate::resumable_effects::source_checkpoint::SourceCheckpointKey;
use rustix::fs::{AtFlags, Mode, OFlags};
use serde_json::{json, Value};
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::AsFd;

/// The immutable record schema for the aggregate durable carrier.
pub(super) const RESUMABLE_JOURNAL_SCHEMA_V2: &str = "semaprax.resumable-journal.v2";
/// Fixed Started record discriminant. It is intentionally not accepted by v1.
pub(super) const CHANNEL_ARGUMENTS_CARRIER_V1: &str = "channel_arguments_v1";

const RECORD_DOMAIN_V2: &[u8] = b"semaprax.resumable-journal-record.v2\0";
const NAME_DOMAIN_V2: &[u8] = b"semaprax.resumable-journal-name.v2\0";

/// Digest aggregate invocation arguments in their canonical channel projection.
/// A newline terminates every argument, preserving the established scalar JSON
/// bytes while preventing adjacent JSON values from being concatenated ambiguously.
pub(super) fn channel_arguments_digest(arguments: &[ResumableChannelValue]) -> [u8; 32] {
    let mut bytes = Vec::new();
    for argument in arguments {
        bytes.extend_from_slice(channel_json(argument).to_string().as_bytes());
        bytes.push(b'\n');
    }
    sha256(&bytes)
}

/// The v2 name domain gives aggregate journals a disjoint owner-private path.
pub(super) fn channel_journal_name(invocation_id: &str) -> String {
    let mut bytes = NAME_DOMAIN_V2.to_vec();
    bytes.extend_from_slice(invocation_id.as_bytes());
    format!("{}.journal", hex(&sha256(&bytes)))
}

/// One verified v2 aggregate-carrier journal record.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum ChannelRecord {
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
        result: ResumableChannelValue,
    },
    Failed {
        class: String,
    },
    CleanupStarted,
    CleanupSettled {
        settlement: String,
    },
}

/// An append-only v2 journal. The v1 [`super::Journal`] API remains scalar.
pub(super) struct ChannelJournal {
    file: File,
    next_seq: u64,
    prev: [u8; 32],
    #[cfg(test)]
    fault: Option<super::super::tests::Fault>,
}

fn digest(bytes: &[u8; 32]) -> String {
    format!("sha256:{}", hex(bytes))
}

fn record_json(record: &ChannelRecord) -> Value {
    match record {
        ChannelRecord::Started {
            function,
            program_digest,
            invocation_id,
            policy_epoch,
            arguments_digest,
            yield_count,
            max_steps,
        } => json!({
            "kind": "started",
            "contract": super::super::RESUMABLE_CONTINUATION_CONTRACT_V1,
            "carrier": CHANNEL_ARGUMENTS_CARRIER_V1,
            "function": function,
            "program_digest": digest(program_digest),
            "invocation_id": invocation_id,
            "policy_epoch": policy_epoch,
            "arguments_digest": digest(arguments_digest),
            "yield_count": yield_count,
            "max_steps": max_steps,
        }),
        ChannelRecord::Yielded {
            site,
            envelope,
            envelope_digest,
        } => json!({
            "kind": "yielded", "site": site, "envelope": envelope,
            "envelope_digest": digest(envelope_digest),
        }),
        ChannelRecord::Dispatched {
            site,
            envelope_digest,
        } => json!({
            "kind": "dispatched", "site": site, "envelope_digest": digest(envelope_digest),
        }),
        ChannelRecord::Answered {
            site,
            envelope_digest,
            answer,
            answer_digest,
        } => json!({
            "kind": "answered", "site": site, "envelope_digest": digest(envelope_digest),
            "answer": channel_json(answer), "answer_digest": digest(answer_digest),
        }),
        ChannelRecord::Completed { result } => {
            json!({"kind": "completed", "result": channel_json(result)})
        }
        ChannelRecord::Failed { class } => json!({"kind": "failed", "class": class}),
        ChannelRecord::CleanupStarted => json!({"kind": "cleanup_started"}),
        ChannelRecord::CleanupSettled { settlement } => {
            json!({"kind": "cleanup_settled", "settlement": settlement})
        }
    }
}

fn site(value: &Value) -> Result<u32, ContinuationError> {
    value["site"]
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(ContinuationError::TamperedJournal)
}

fn parse_record(value: &Value) -> Result<ChannelRecord, ContinuationError> {
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
                    "carrier",
                    "function",
                    "program_digest",
                    "invocation_id",
                    "policy_epoch",
                    "arguments_digest",
                    "yield_count",
                    "max_steps",
                ],
            )?;
            if value["contract"] != super::super::RESUMABLE_CONTINUATION_CONTRACT_V1
                || value["carrier"] != CHANNEL_ARGUMENTS_CARRIER_V1
            {
                return Err(ContinuationError::SchemaMismatch);
            }
            ChannelRecord::Started {
                function: text(value, "function")?,
                program_digest: digest_field(&value["program_digest"])?,
                invocation_id: text(value, "invocation_id")?,
                policy_epoch: value["policy_epoch"]
                    .as_u64()
                    .ok_or(ContinuationError::TamperedJournal)?,
                arguments_digest: digest_field(&value["arguments_digest"])?,
                yield_count: value["yield_count"]
                    .as_u64()
                    .and_then(|value| u32::try_from(value).ok())
                    .ok_or(ContinuationError::TamperedJournal)?,
                max_steps: value["max_steps"]
                    .as_u64()
                    .ok_or(ContinuationError::TamperedJournal)?,
            }
        }
        "yielded" => {
            exact_keys(value, &["kind", "site", "envelope", "envelope_digest"])?;
            ChannelRecord::Yielded {
                site: site(value)?,
                envelope: text(value, "envelope")?,
                envelope_digest: digest_field(&value["envelope_digest"])?,
            }
        }
        "dispatched" => {
            exact_keys(value, &["kind", "site", "envelope_digest"])?;
            ChannelRecord::Dispatched {
                site: site(value)?,
                envelope_digest: digest_field(&value["envelope_digest"])?,
            }
        }
        "answered" => {
            exact_keys(
                value,
                &["kind", "site", "envelope_digest", "answer", "answer_digest"],
            )?;
            ChannelRecord::Answered {
                site: site(value)?,
                envelope_digest: digest_field(&value["envelope_digest"])?,
                answer: channel(&value["answer"])?,
                answer_digest: digest_field(&value["answer_digest"])?,
            }
        }
        "completed" => {
            exact_keys(value, &["kind", "result"])?;
            ChannelRecord::Completed {
                result: channel(&value["result"])?,
            }
        }
        "failed" => {
            exact_keys(value, &["kind", "class"])?;
            ChannelRecord::Failed {
                class: text(value, "class")?,
            }
        }
        "cleanup_started" => {
            exact_keys(value, &["kind"])?;
            ChannelRecord::CleanupStarted
        }
        "cleanup_settled" => {
            exact_keys(value, &["kind", "settlement"])?;
            ChannelRecord::CleanupSettled {
                settlement: text(value, "settlement")?,
            }
        }
        _ => return Err(ContinuationError::TamperedJournal),
    };
    if record_json(&record) != *value {
        return Err(ContinuationError::TamperedJournal);
    }
    Ok(record)
}

fn unsigned(seq: u64, prev: &[u8; 32], record: &ChannelRecord) -> Value {
    json!({
        "schema": RESUMABLE_JOURNAL_SCHEMA_V2, "seq": seq, "prev": digest(prev),
        "record": record_json(record),
    })
}

fn render_line(
    key: &SourceCheckpointKey,
    seq: u64,
    prev: &[u8; 32],
    record: &ChannelRecord,
) -> String {
    let payload = unsigned(seq, prev, record);
    let tag = key.authenticate(RECORD_DOMAIN_V2, payload.to_string().as_bytes());
    json!({
        "schema": RESUMABLE_JOURNAL_SCHEMA_V2, "seq": seq, "prev": digest(prev),
        "record": payload["record"].clone(), "mac": format!("hmac-sha256:{}", hex(&tag)),
    })
    .to_string()
}

impl ChannelJournal {
    /// Create a disjoint v2 aggregate journal without overwriting any v1 path.
    pub(super) fn create(
        directory: &JournalDirectory,
        invocation_id: &str,
    ) -> Result<Self, ContinuationError> {
        let name = channel_journal_name(invocation_id);
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
            fault: super::super::tests::armed_fault(),
        })
    }

    /// Recover only v2 lines under the v2 MAC domain; v1 is schema-refused.
    pub(super) fn open(
        directory: &JournalDirectory,
        invocation_id: &str,
        key: &SourceCheckpointKey,
        torn: TornTailPolicy,
    ) -> Result<(Self, Vec<ChannelRecord>), ContinuationError> {
        let fd = rustix::fs::openat(
            &directory.fd,
            channel_journal_name(invocation_id),
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
            fault: super::super::tests::armed_fault(),
        };
        let mut records = Vec::new();
        for chunk in bytes[..acknowledged].split_inclusive(|byte| *byte == b'\n') {
            let line = &chunk[..chunk.len() - 1];
            if line.is_empty() || records.len() == MAX_RECORDS {
                return Err(ContinuationError::TamperedJournal);
            }
            records.push(journal.verify_line(key, line)?);
        }
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
    ) -> Result<ChannelRecord, ContinuationError> {
        let document: Value =
            serde_json::from_slice(line).map_err(|_| ContinuationError::TamperedJournal)?;
        exact_keys(&document, &["schema", "seq", "prev", "record", "mac"])?;
        if document["schema"] != RESUMABLE_JOURNAL_SCHEMA_V2 {
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
        if !key.verify(RECORD_DOMAIN_V2, payload.to_string().as_bytes(), &tag)
            || render_line(key, self.next_seq, &self.prev, &record).as_bytes() != line
        {
            return Err(ContinuationError::TamperedJournal);
        }
        self.next_seq += 1;
        self.prev = sha256(line);
        Ok(record)
    }

    /// Append a v2 record atomically. A returned `Ok` is the durable acknowledgement.
    pub(super) fn append(
        &mut self,
        key: &SourceCheckpointKey,
        record: &ChannelRecord,
    ) -> Result<(), ContinuationError> {
        if self.next_seq as usize >= MAX_RECORDS {
            return Err(ContinuationError::TamperedJournal);
        }
        #[cfg(test)]
        let after = match self.fault.as_mut().map(|fault| fault.before_append()) {
            Some(super::super::tests::FaultAction::CrashBefore) => {
                return Err(ContinuationError::Storage)
            }
            Some(super::super::tests::FaultAction::CrashAfter) => true,
            _ => false,
        };
        let line = render_line(key, self.next_seq, &self.prev, record);
        let mut bytes = line.clone().into_bytes();
        bytes.push(b'\n');
        let size = rustix::fs::fstat(self.file.as_fd()).map_err(io)?.st_size;
        if u64::try_from(size).map_err(io)? + bytes.len() as u64 > MAX_JOURNAL_BYTES as u64 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn scratch() -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "spx-channel-journal-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }

    fn key() -> SourceCheckpointKey {
        SourceCheckpointKey::new([0x52; 32])
    }

    #[test]
    fn arguments_digest_uses_channel_json_with_one_lf_per_argument() {
        let arguments = vec![
            ResumableChannelValue::Scalar(crate::interpreter::ArgumentValue::Int(7)),
            ResumableChannelValue::Record {
                declaration: crate::hir::DeclarationId::new("app.input"),
                fields: vec![crate::interpreter::ArgumentValue::Bool(true)],
            },
        ];
        let expected = b"7\n{\"declaration\":\"app.input\",\"fields\":[true],\"tag\":\"record\"}\n";
        assert_eq!(channel_arguments_digest(&arguments), sha256(expected));
    }

    #[test]
    fn v2_round_trip_preserves_aggregate_completed_value() {
        let path = scratch();
        let directory = JournalDirectory::open(&path).unwrap();
        let key = key();
        let result = ResumableChannelValue::Record {
            declaration: crate::hir::DeclarationId::new("app.output"),
            fields: vec![crate::interpreter::ArgumentValue::Int(9)],
        };
        let mut journal = ChannelJournal::create(&directory, "aggregate-v2").unwrap();
        journal
            .append(
                &key,
                &ChannelRecord::Started {
                    function: "app.ask".into(),
                    program_digest: [3; 32],
                    invocation_id: "aggregate-v2".into(),
                    policy_epoch: 4,
                    arguments_digest: channel_arguments_digest(&[result.clone()]),
                    yield_count: 1,
                    max_steps: 100,
                },
            )
            .unwrap();
        journal
            .append(
                &key,
                &ChannelRecord::Completed {
                    result: result.clone(),
                },
            )
            .unwrap();
        drop(journal);
        let (_, records) =
            ChannelJournal::open(&directory, "aggregate-v2", &key, TornTailPolicy::Refuse).unwrap();
        assert!(
            matches!(records.as_slice(), [ChannelRecord::Started { .. }, ChannelRecord::Completed { result: observed }] if observed == &result)
        );
        drop(directory);
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn v2_refuses_v1_schema_at_its_own_disjoint_path() {
        let path = scratch();
        let name = channel_journal_name("cross-schema");
        let document = json!({
            "schema": super::super::RESUMABLE_JOURNAL_SCHEMA_V1,
            "seq": 0,
            "prev": format!("sha256:{}", hex(&[0; 32])),
            "record": {"kind": "cleanup_started"},
            "mac": format!("hmac-sha256:{}", hex(&[0; 32])),
        });
        fs::write(path.join(name), format!("{document}\n")).unwrap();
        fs::set_permissions(
            path.join(channel_journal_name("cross-schema")),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        let directory = JournalDirectory::open(&path).unwrap();
        assert!(matches!(
            ChannelJournal::open(&directory, "cross-schema", &key(), TornTailPolicy::Refuse),
            Err(ContinuationError::SchemaMismatch)
        ));
        drop(directory);
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn v2_torn_tail_is_refused_or_explicitly_truncated() {
        let path = scratch();
        let directory = JournalDirectory::open(&path).unwrap();
        let key = key();
        let mut journal = ChannelJournal::create(&directory, "torn-v2").unwrap();
        journal
            .append(&key, &ChannelRecord::CleanupStarted)
            .unwrap();
        drop(journal);
        let journal_path = path.join(channel_journal_name("torn-v2"));
        std::io::Write::write_all(
            &mut fs::OpenOptions::new()
                .append(true)
                .open(&journal_path)
                .unwrap(),
            b"unacknowledged",
        )
        .unwrap();
        assert!(matches!(
            ChannelJournal::open(&directory, "torn-v2", &key, TornTailPolicy::Refuse),
            Err(ContinuationError::TornTail)
        ));
        let (_, records) = ChannelJournal::open(
            &directory,
            "torn-v2",
            &key,
            TornTailPolicy::TruncateUnacknowledged,
        )
        .unwrap();
        assert_eq!(records, vec![ChannelRecord::CleanupStarted]);
        drop(directory);
        let _ = fs::remove_dir_all(path);
    }
}
