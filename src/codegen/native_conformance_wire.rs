//! Test-only binary transport for native conformance probes.
//!
//! The wire format is deliberately independent of C struct layout and JSON:
//!
//! ```text
//! magic[8] = "SPXTRC1\0"
//! version:u32-le
//! event_count:u32-le
//! scenario:utf8
//! root_function:identity
//! events[event_count]
//! outcome
//! ```
//!
//! Every tag, count, index, status code, and flag is a fixed-width little-endian
//! `u32`. Strings are a `u32` byte length followed by exact UTF-8 bytes. The
//! decoder rejects unknown tags, truncation, trailing bytes, embedded NUL in
//! semantic identities, and inputs outside explicit resource limits.

use std::error::Error;
use std::fmt;

pub(super) const MAGIC: &[u8; 8] = b"SPXTRC1\0";
pub(super) const VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DecodeLimits {
    pub max_frame_bytes: usize,
    pub max_events: usize,
    pub max_collection_items: usize,
    pub max_string_bytes: usize,
    pub max_total_string_bytes: usize,
}

pub(super) const DEFAULT_LIMITS: DecodeLimits = DecodeLimits {
    max_frame_bytes: 16 * 1024 * 1024,
    max_events: 65_536,
    max_collection_items: 262_144,
    max_string_bytes: 4_096,
    max_total_string_bytes: 4 * 1024 * 1024,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum WireStorage {
    Value {
        value_id: String,
    },
    Temporary {
        expression_id: String,
    },
    CallArgument {
        call_id: String,
        parameter_index: u32,
        value_expression_id: String,
    },
    ProvisionalResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WirePlace {
    pub storage: WireStorage,
    pub projections: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WireStatusLane {
    OperationFailure,
    ContractFalse,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WireStatusSource {
    pub expression_id: String,
    pub lane: WireStatusLane,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WireStatusClass {
    Contract,
    Arithmetic,
    Import,
    ExplicitClose,
    Adapter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WireRetryability {
    Unknown,
    False,
    True,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WireStatus {
    pub schema: String,
    pub domain_id: String,
    pub code: u32,
    pub class: WireStatusClass,
    pub retryability: WireRetryability,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum WireResultSource {
    Scalar { expression_id: String },
    Owned { storage: WirePlace },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum WireEventKind {
    Transfer {
        at: String,
        source: WirePlace,
        destination: WirePlace,
    },
    SelectFailure {
        source: WireStatusSource,
        status: WireStatus,
    },
    FinalizeBegin {
        source: WirePlace,
        lifecycle_id: String,
        guard_flag: u32,
        binding_import_id: Option<String>,
    },
    FinalizeEnd {
        source: WirePlace,
        lifecycle_id: String,
        guard_flag: u32,
        binding_import_id: Option<String>,
    },
    ResultCommit {
        source: WireResultSource,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WireEvent {
    pub function_id: String,
    pub invocation: Vec<String>,
    pub kind: WireEventKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum WireResult {
    I64(i64),
    Bool(bool),
    Unit,
    Owned { type_id: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum WireOutcome {
    Success(WireResult),
    Failure {
        selected_source: WireStatusSource,
        status: WireStatus,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WireTrace {
    pub scenario_id: String,
    pub root_function_id: String,
    pub events: Vec<WireEvent>,
    pub outcome: WireOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum WireDecodeError {
    FrameTooLarge { actual: usize, maximum: usize },
    Truncated { offset: usize, needed: usize },
    BadMagic,
    UnsupportedVersion(u32),
    UnknownTag { context: &'static str, tag: u32 },
    InvalidUtf8 { offset: usize },
    IdentityContainsNul { offset: usize },
    InvalidStatus(&'static str),
    StringTooLarge { actual: usize, maximum: usize },
    StringBudgetExceeded { maximum: usize },
    EventLimitExceeded { actual: usize, maximum: usize },
    CollectionLimitExceeded { maximum: usize },
    LengthOverflow,
    AllocationFailed,
    TrailingBytes { count: usize },
}

impl fmt::Display for WireDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FrameTooLarge { actual, maximum } => {
                write!(
                    formatter,
                    "wire frame has {actual} bytes; maximum is {maximum}"
                )
            }
            Self::Truncated { offset, needed } => write!(
                formatter,
                "wire frame is truncated at byte {offset}; needs {needed} more bytes"
            ),
            Self::BadMagic => formatter.write_str("wire frame has an invalid magic header"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported wire version {version}")
            }
            Self::UnknownTag { context, tag } => {
                write!(formatter, "unknown {context} tag {tag}")
            }
            Self::InvalidUtf8 { offset } => {
                write!(formatter, "wire string at byte {offset} is not valid UTF-8")
            }
            Self::IdentityContainsNul { offset } => write!(
                formatter,
                "wire semantic identity at byte {offset} contains NUL"
            ),
            Self::InvalidStatus(reason) => write!(formatter, "invalid wire status: {reason}"),
            Self::StringTooLarge { actual, maximum } => {
                write!(
                    formatter,
                    "wire string has {actual} bytes; maximum is {maximum}"
                )
            }
            Self::StringBudgetExceeded { maximum } => {
                write!(
                    formatter,
                    "wire strings exceed the aggregate {maximum}-byte limit"
                )
            }
            Self::EventLimitExceeded { actual, maximum } => {
                write!(
                    formatter,
                    "wire frame has {actual} events; maximum is {maximum}"
                )
            }
            Self::CollectionLimitExceeded { maximum } => write!(
                formatter,
                "wire collections exceed the aggregate {maximum}-item limit"
            ),
            Self::LengthOverflow => formatter.write_str("wire length arithmetic overflowed"),
            Self::AllocationFailed => formatter.write_str("wire decode allocation failed"),
            Self::TrailingBytes { count } => {
                write!(formatter, "wire frame contains {count} trailing bytes")
            }
        }
    }
}

impl Error for WireDecodeError {}

pub(super) fn decode(bytes: &[u8]) -> Result<WireTrace, WireDecodeError> {
    decode_with_limits(bytes, DEFAULT_LIMITS)
}

pub(super) fn decode_with_limits(
    bytes: &[u8],
    limits: DecodeLimits,
) -> Result<WireTrace, WireDecodeError> {
    if bytes.len() > limits.max_frame_bytes {
        return Err(WireDecodeError::FrameTooLarge {
            actual: bytes.len(),
            maximum: limits.max_frame_bytes,
        });
    }

    let mut reader = Reader::new(bytes, limits);
    if reader.read_exact(MAGIC.len())? != MAGIC {
        return Err(WireDecodeError::BadMagic);
    }
    let version = reader.read_u32()?;
    if version != VERSION {
        return Err(WireDecodeError::UnsupportedVersion(version));
    }

    let event_count = reader.read_count()?;
    if event_count > limits.max_events {
        return Err(WireDecodeError::EventLimitExceeded {
            actual: event_count,
            maximum: limits.max_events,
        });
    }
    reader.claim_items(event_count)?;

    let scenario_id = reader.read_identity()?;
    let root_function_id = reader.read_identity()?;
    reader.ensure_remaining_for(event_count, 12, 8)?;
    let mut events = reader.collection(event_count)?;
    for _ in 0..event_count {
        events.push(reader.read_event()?);
    }
    let outcome = reader.read_outcome()?;

    if reader.position != bytes.len() {
        return Err(WireDecodeError::TrailingBytes {
            count: bytes.len() - reader.position,
        });
    }

    Ok(WireTrace {
        scenario_id,
        root_function_id,
        events,
        outcome,
    })
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
    limits: DecodeLimits,
    collection_items: usize,
    string_bytes: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8], limits: DecodeLimits) -> Self {
        Self {
            bytes,
            position: 0,
            limits,
            collection_items: 0,
            string_bytes: 0,
        }
    }

    fn read_exact(&mut self, length: usize) -> Result<&'a [u8], WireDecodeError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(WireDecodeError::LengthOverflow)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(WireDecodeError::Truncated {
                offset: self.position,
                needed: end.saturating_sub(self.bytes.len()),
            })?;
        self.position = end;
        Ok(value)
    }

    fn read_u32(&mut self) -> Result<u32, WireDecodeError> {
        let bytes: [u8; 4] = self
            .read_exact(4)?
            .try_into()
            .map_err(|_| WireDecodeError::LengthOverflow)?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn read_i64(&mut self) -> Result<i64, WireDecodeError> {
        let bytes: [u8; 8] = self
            .read_exact(8)?
            .try_into()
            .map_err(|_| WireDecodeError::LengthOverflow)?;
        Ok(i64::from_le_bytes(bytes))
    }

    fn read_count(&mut self) -> Result<usize, WireDecodeError> {
        usize::try_from(self.read_u32()?).map_err(|_| WireDecodeError::LengthOverflow)
    }

    fn read_tag(&mut self, context: &'static str) -> Result<u32, WireDecodeError> {
        let tag = self.read_u32()?;
        if tag == 0 {
            return Err(WireDecodeError::UnknownTag { context, tag });
        }
        Ok(tag)
    }

    fn read_text(&mut self) -> Result<String, WireDecodeError> {
        let offset = self.position;
        let length = self.read_count()?;
        if length > self.limits.max_string_bytes {
            return Err(WireDecodeError::StringTooLarge {
                actual: length,
                maximum: self.limits.max_string_bytes,
            });
        }
        self.string_bytes = self
            .string_bytes
            .checked_add(length)
            .ok_or(WireDecodeError::LengthOverflow)?;
        if self.string_bytes > self.limits.max_total_string_bytes {
            return Err(WireDecodeError::StringBudgetExceeded {
                maximum: self.limits.max_total_string_bytes,
            });
        }
        let bytes = self.read_exact(length)?;
        let text =
            std::str::from_utf8(bytes).map_err(|_| WireDecodeError::InvalidUtf8 { offset })?;
        Ok(text.to_owned())
    }

    fn read_identity(&mut self) -> Result<String, WireDecodeError> {
        let offset = self.position;
        let identity = self.read_text()?;
        if identity.as_bytes().contains(&0) {
            return Err(WireDecodeError::IdentityContainsNul { offset });
        }
        Ok(identity)
    }

    fn claim_items(&mut self, count: usize) -> Result<(), WireDecodeError> {
        self.collection_items = self
            .collection_items
            .checked_add(count)
            .ok_or(WireDecodeError::LengthOverflow)?;
        if self.collection_items > self.limits.max_collection_items {
            return Err(WireDecodeError::CollectionLimitExceeded {
                maximum: self.limits.max_collection_items,
            });
        }
        Ok(())
    }

    fn collection<T>(&self, count: usize) -> Result<Vec<T>, WireDecodeError> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| WireDecodeError::AllocationFailed)?;
        Ok(values)
    }

    fn ensure_remaining_for(
        &self,
        count: usize,
        bytes_per_item: usize,
        trailing_bytes: usize,
    ) -> Result<(), WireDecodeError> {
        let needed = count
            .checked_mul(bytes_per_item)
            .and_then(|value| value.checked_add(trailing_bytes))
            .ok_or(WireDecodeError::LengthOverflow)?;
        let remaining = self.bytes.len().saturating_sub(self.position);
        if remaining < needed {
            return Err(WireDecodeError::Truncated {
                offset: self.position,
                needed: needed - remaining,
            });
        }
        Ok(())
    }

    fn read_identities(&mut self) -> Result<Vec<String>, WireDecodeError> {
        let count = self.read_count()?;
        self.claim_items(count)?;
        self.ensure_remaining_for(count, 4, 0)?;
        let mut values = self.collection(count)?;
        for _ in 0..count {
            values.push(self.read_identity()?);
        }
        Ok(values)
    }

    fn read_event(&mut self) -> Result<WireEvent, WireDecodeError> {
        let tag = self.read_tag("event")?;
        if !matches!(tag, 2 | 6 | 7 | 8 | 9) {
            return Err(WireDecodeError::UnknownTag {
                context: "event",
                tag,
            });
        }
        let function_id = self.read_identity()?;
        let invocation = self.read_identities()?;
        let kind = match tag {
            2 => WireEventKind::Transfer {
                at: self.read_identity()?,
                source: self.read_place()?,
                destination: self.read_place()?,
            },
            6 => WireEventKind::SelectFailure {
                source: self.read_status_source()?,
                status: self.read_status()?,
            },
            7 => WireEventKind::FinalizeBegin {
                source: self.read_place()?,
                lifecycle_id: self.read_identity()?,
                guard_flag: self.read_u32()?,
                binding_import_id: self.read_optional_identity()?,
            },
            8 => WireEventKind::FinalizeEnd {
                source: self.read_place()?,
                lifecycle_id: self.read_identity()?,
                guard_flag: self.read_u32()?,
                binding_import_id: self.read_optional_identity()?,
            },
            9 => WireEventKind::ResultCommit {
                source: self.read_result_source()?,
            },
            _ => {
                return Err(WireDecodeError::UnknownTag {
                    context: "event",
                    tag,
                })
            }
        };
        Ok(WireEvent {
            function_id,
            invocation,
            kind,
        })
    }

    fn read_storage(&mut self) -> Result<WireStorage, WireDecodeError> {
        let tag = self.read_tag("storage")?;
        match tag {
            1 => Ok(WireStorage::Value {
                value_id: self.read_identity()?,
            }),
            2 => Ok(WireStorage::Temporary {
                expression_id: self.read_identity()?,
            }),
            3 => Ok(WireStorage::CallArgument {
                call_id: self.read_identity()?,
                parameter_index: self.read_u32()?,
                value_expression_id: self.read_identity()?,
            }),
            4 => Ok(WireStorage::ProvisionalResult),
            _ => Err(WireDecodeError::UnknownTag {
                context: "storage",
                tag,
            }),
        }
    }

    fn read_place(&mut self) -> Result<WirePlace, WireDecodeError> {
        Ok(WirePlace {
            storage: self.read_storage()?,
            projections: self.read_identities()?,
        })
    }

    fn read_status_source(&mut self) -> Result<WireStatusSource, WireDecodeError> {
        let expression_id = self.read_identity()?;
        let lane = match self.read_tag("status lane")? {
            1 => WireStatusLane::OperationFailure,
            2 => WireStatusLane::ContractFalse,
            tag => {
                return Err(WireDecodeError::UnknownTag {
                    context: "status lane",
                    tag,
                });
            }
        };
        Ok(WireStatusSource {
            expression_id,
            lane,
        })
    }

    fn read_status(&mut self) -> Result<WireStatus, WireDecodeError> {
        let schema = self.read_text()?;
        let domain_id = self.read_text()?;
        let code = self.read_u32()?;
        let class = match self.read_tag("status class")? {
            1 => WireStatusClass::Contract,
            2 => WireStatusClass::Arithmetic,
            3 => WireStatusClass::Import,
            4 => WireStatusClass::ExplicitClose,
            5 => WireStatusClass::Adapter,
            tag => {
                return Err(WireDecodeError::UnknownTag {
                    context: "status class",
                    tag,
                });
            }
        };
        let retryability = match self.read_u32()? {
            0 => WireRetryability::Unknown,
            1 => WireRetryability::False,
            2 => WireRetryability::True,
            tag => {
                return Err(WireDecodeError::UnknownTag {
                    context: "retryability",
                    tag,
                });
            }
        };
        let status = WireStatus {
            schema,
            domain_id,
            code,
            class,
            retryability,
        };
        validate_status(&status)?;
        Ok(status)
    }

    fn read_result_source(&mut self) -> Result<WireResultSource, WireDecodeError> {
        let tag = self.read_tag("result source")?;
        match tag {
            1 => Ok(WireResultSource::Scalar {
                expression_id: self.read_identity()?,
            }),
            2 => Ok(WireResultSource::Owned {
                storage: self.read_place()?,
            }),
            _ => Err(WireDecodeError::UnknownTag {
                context: "result source",
                tag,
            }),
        }
    }

    fn read_optional_identity(&mut self) -> Result<Option<String>, WireDecodeError> {
        match self.read_u32()? {
            0 => Ok(None),
            1 => Ok(Some(self.read_identity()?)),
            tag => Err(WireDecodeError::UnknownTag {
                context: "optional identity",
                tag,
            }),
        }
    }

    fn read_outcome(&mut self) -> Result<WireOutcome, WireDecodeError> {
        let tag = self.read_tag("trace outcome")?;
        match tag {
            1 => Ok(WireOutcome::Success(self.read_result()?)),
            2 => Ok(WireOutcome::Failure {
                selected_source: self.read_status_source()?,
                status: self.read_status()?,
            }),
            _ => Err(WireDecodeError::UnknownTag {
                context: "trace outcome",
                tag,
            }),
        }
    }

    fn read_result(&mut self) -> Result<WireResult, WireDecodeError> {
        let tag = self.read_tag("trace result")?;
        match tag {
            1 => Ok(WireResult::I64(self.read_i64()?)),
            2 => match self.read_u32()? {
                0 => Ok(WireResult::Bool(false)),
                1 => Ok(WireResult::Bool(true)),
                tag => Err(WireDecodeError::UnknownTag {
                    context: "boolean",
                    tag,
                }),
            },
            3 => Ok(WireResult::Unit),
            4 => Ok(WireResult::Owned {
                type_id: self.read_identity()?,
            }),
            _ => Err(WireDecodeError::UnknownTag {
                context: "trace result",
                tag,
            }),
        }
    }
}

fn validate_status(status: &WireStatus) -> Result<(), WireDecodeError> {
    const STATUS_SCHEMA: &str = "semaprax.status.v1";
    const CONTRACT_DOMAIN: &str = "semaprax.contract.v1";
    const ARITHMETIC_DOMAIN: &str = "semaprax.arithmetic.v1";
    const MAX_DOMAIN_BYTES: usize = 255;

    if status.schema != STATUS_SCHEMA {
        return Err(WireDecodeError::InvalidStatus(
            "schema must be semaprax.status.v1",
        ));
    }
    if status.domain_id.is_empty() {
        return Err(WireDecodeError::InvalidStatus(
            "domain identity cannot be empty",
        ));
    }
    if status.domain_id.len() > MAX_DOMAIN_BYTES {
        return Err(WireDecodeError::InvalidStatus(
            "domain identity cannot exceed 255 UTF-8 bytes",
        ));
    }
    if status.domain_id.as_bytes().contains(&0) {
        return Err(WireDecodeError::InvalidStatus(
            "domain identity cannot contain NUL",
        ));
    }
    if status.code == 0 {
        return Err(WireDecodeError::InvalidStatus(
            "status code zero is reserved for success",
        ));
    }
    if status.retryability != WireRetryability::False {
        return Err(WireDecodeError::InvalidStatus(
            "compiler-owned statuses must have retryability false",
        ));
    }

    match status.class {
        WireStatusClass::Contract => {
            if status.domain_id != CONTRACT_DOMAIN || !(1..=2).contains(&status.code) {
                return Err(WireDecodeError::InvalidStatus(
                    "contract class requires semaprax.contract.v1 and code 1 or 2",
                ));
            }
        }
        WireStatusClass::Arithmetic => {
            if status.domain_id != ARITHMETIC_DOMAIN || !(1..=8).contains(&status.code) {
                return Err(WireDecodeError::InvalidStatus(
                    "arithmetic class requires semaprax.arithmetic.v1 and a StatusCase code 1 through 8",
                ));
            }
        }
        WireStatusClass::Import | WireStatusClass::ExplicitClose | WireStatusClass::Adapter => {
            return Err(WireDecodeError::InvalidStatus(
                "external status classes are outside the current native trace slice",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Writer(Vec<u8>);

    impl Writer {
        fn new(event_count: u32, scenario: &str, root: &str) -> Self {
            let mut writer = Self(Vec::new());
            writer.0.extend_from_slice(MAGIC);
            writer.u32(VERSION);
            writer.u32(event_count);
            writer.text(scenario);
            writer.text(root);
            writer
        }

        fn u32(&mut self, value: u32) {
            self.0.extend_from_slice(&value.to_le_bytes());
        }

        fn text(&mut self, value: &str) {
            self.u32(value.len().try_into().unwrap());
            self.0.extend_from_slice(value.as_bytes());
        }

        fn storage(&mut self, tag: u32, first: &str) {
            self.u32(tag);
            if tag == 1 || tag == 2 {
                self.text(first);
            }
        }

        fn place(&mut self, storage_tag: u32, id: &str, projections: &[&str]) {
            self.storage(storage_tag, id);
            self.u32(projections.len().try_into().unwrap());
            for projection in projections {
                self.text(projection);
            }
        }

        fn status(&mut self, class: u32, retryability: u32) {
            let (domain, code) = match class {
                1 => ("semaprax.contract.v1", 1),
                2 => ("semaprax.arithmetic.v1", 8),
                _ => ("example.domain", 41),
            };
            self.raw_status("semaprax.status.v1", domain, code, class, retryability);
        }

        fn raw_status(
            &mut self,
            schema: &str,
            domain: &str,
            code: u32,
            class: u32,
            retryability: u32,
        ) {
            self.text(schema);
            self.text(domain);
            self.u32(code);
            self.u32(class);
            self.u32(retryability);
        }

        fn event_header(&mut self, tag: u32, function: &str, invocation: &[&str]) {
            self.u32(tag);
            self.text(function);
            self.u32(invocation.len().try_into().unwrap());
            for expression in invocation {
                self.text(expression);
            }
        }

        fn success_unit(&mut self) {
            self.u32(1);
            self.u32(3);
        }
    }

    fn all_event_frame() -> Vec<u8> {
        let mut writer = Writer::new(5, "scenario-🦀", "fn.root");

        writer.event_header(2, "fn.root", &["expr.call"]);
        writer.text("expr.move");
        writer.place(2, "expr.source", &["field.inner"]);
        writer.place(4, "", &[]);

        writer.event_header(6, "fn.root", &[]);
        writer.text("expr.contract");
        writer.u32(2);
        writer.status(1, 1);

        writer.event_header(7, "fn.root", &[]);
        writer.place(1, "value.resource", &[]);
        writer.text("life.resource");
        writer.u32(9);
        writer.u32(1);
        writer.text("import.drop");

        writer.event_header(8, "fn.root", &[]);
        writer.place(1, "value.resource", &[]);
        writer.text("life.resource");
        writer.u32(9);
        writer.u32(0);

        writer.event_header(9, "fn.root", &[]);
        writer.u32(2);
        writer.place(4, "", &[]);

        writer.success_unit();
        writer.0
    }

    fn select_failure_frame(
        schema: &str,
        domain: &str,
        code: u32,
        class: u32,
        retryability: u32,
    ) -> Vec<u8> {
        let mut writer = Writer::new(1, "hostile-status", "fn.root");
        writer.event_header(6, "fn.root", &[]);
        writer.text("expr.failed");
        writer.u32(1);
        writer.raw_status(schema, domain, code, class, retryability);
        writer.success_unit();
        writer.0
    }

    #[test]
    fn decodes_every_event_variant_and_nested_shape() {
        let trace = decode(&all_event_frame()).unwrap();
        assert_eq!(trace.scenario_id, "scenario-🦀");
        assert_eq!(trace.root_function_id, "fn.root");
        assert_eq!(trace.events.len(), 5);
        assert!(matches!(
            &trace.events[0].kind,
            WireEventKind::Transfer { at, source, destination }
                if at == "expr.move"
                    && source.projections == ["field.inner"]
                    && matches!(destination.storage, WireStorage::ProvisionalResult)
        ));
        assert!(matches!(
            &trace.events[1].kind,
            WireEventKind::SelectFailure {
                source: WireStatusSource {
                    lane: WireStatusLane::ContractFalse,
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            trace.outcome,
            WireOutcome::Success(WireResult::Unit)
        ));
    }

    #[test]
    fn decodes_all_trace_results_and_failure_outcome() {
        let cases = [
            (
                1,
                i64::MIN.to_le_bytes().to_vec(),
                WireResult::I64(i64::MIN),
            ),
            (2, 1_u32.to_le_bytes().to_vec(), WireResult::Bool(true)),
            (3, Vec::new(), WireResult::Unit),
        ];
        for (tag, payload, expected) in cases {
            let mut writer = Writer::new(0, "scenario", "fn.root");
            writer.u32(1);
            writer.u32(tag);
            writer.0.extend_from_slice(&payload);
            assert_eq!(
                decode(&writer.0).unwrap().outcome,
                WireOutcome::Success(expected)
            );
        }

        let mut owned = Writer::new(0, "scenario", "fn.root");
        owned.u32(1);
        owned.u32(4);
        owned.text("resource.Token");
        assert_eq!(
            decode(&owned.0).unwrap().outcome,
            WireOutcome::Success(WireResult::Owned {
                type_id: "resource.Token".into()
            })
        );

        let mut failure = Writer::new(0, "scenario", "fn.root");
        failure.u32(2);
        failure.text("expr.failed");
        failure.u32(1);
        failure.status(2, 1);
        assert!(matches!(
            decode(&failure.0).unwrap().outcome,
            WireOutcome::Failure {
                selected_source: WireStatusSource {
                    lane: WireStatusLane::OperationFailure,
                    ..
                },
                status: WireStatus {
                    class: WireStatusClass::Arithmetic,
                    retryability: WireRetryability::False,
                    ..
                }
            }
        ));
    }

    #[test]
    fn every_proper_prefix_is_rejected_as_truncated() {
        let frame = all_event_frame();
        for end in 0..frame.len() {
            assert!(
                matches!(
                    decode(&frame[..end]),
                    Err(WireDecodeError::Truncated { .. })
                ),
                "prefix ending at {end} was not reported as truncated"
            );
        }
    }

    #[test]
    fn rejects_header_version_unknown_tags_and_trailing_bytes() {
        let mut bad_magic = all_event_frame();
        bad_magic[0] ^= 1;
        assert_eq!(decode(&bad_magic), Err(WireDecodeError::BadMagic));

        let mut bad_version = all_event_frame();
        bad_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(
            decode(&bad_version),
            Err(WireDecodeError::UnsupportedVersion(2))
        );

        let mut unknown_event = Writer::new(1, "s", "f");
        unknown_event.u32(77);
        unknown_event.text("f");
        unknown_event.u32(0);
        unknown_event.success_unit();
        assert_eq!(
            decode(&unknown_event.0),
            Err(WireDecodeError::UnknownTag {
                context: "event",
                tag: 77
            })
        );

        let mut trailing = Writer::new(0, "s", "f");
        trailing.success_unit();
        trailing.0.push(0);
        assert_eq!(
            decode(&trailing.0),
            Err(WireDecodeError::TrailingBytes { count: 1 })
        );
    }

    #[test]
    fn rejects_invalid_utf8_and_nul_identities() {
        let mut invalid_utf8 = Writer::new(0, "s", "f");
        let scenario_offset = 16;
        invalid_utf8.0[scenario_offset + 4] = 0xff;
        assert!(matches!(
            decode(&invalid_utf8.0),
            Err(WireDecodeError::InvalidUtf8 { .. })
        ));

        let mut nul = Writer::new(0, "s", "f\0hostile");
        nul.success_unit();
        assert!(matches!(
            decode(&nul.0),
            Err(WireDecodeError::IdentityContainsNul { .. })
        ));
    }

    #[test]
    fn enforces_frame_event_string_and_collection_limits_before_allocation() {
        let frame = all_event_frame();
        let mut limits = DEFAULT_LIMITS;
        limits.max_frame_bytes = frame.len() - 1;
        assert!(matches!(
            decode_with_limits(&frame, limits),
            Err(WireDecodeError::FrameTooLarge { .. })
        ));

        let mut event_bomb = Writer::new(u32::MAX, "s", "f");
        event_bomb.success_unit();
        let mut limits = DEFAULT_LIMITS;
        limits.max_events = 4;
        assert_eq!(
            decode_with_limits(&event_bomb.0, limits),
            Err(WireDecodeError::EventLimitExceeded {
                actual: u32::MAX as usize,
                maximum: 4
            })
        );

        let mut long_string = Writer::new(0, "five!", "f");
        long_string.success_unit();
        let mut limits = DEFAULT_LIMITS;
        limits.max_string_bytes = 4;
        assert!(matches!(
            decode_with_limits(&long_string.0, limits),
            Err(WireDecodeError::StringTooLarge {
                actual: 5,
                maximum: 4
            })
        ));

        let mut collection_bomb = Writer::new(1, "s", "f");
        collection_bomb.event_header(2, "f", &[]);
        collection_bomb.text("e");
        collection_bomb.storage(4, "");
        collection_bomb.u32(3);
        collection_bomb.success_unit();
        let mut limits = DEFAULT_LIMITS;
        limits.max_collection_items = 2;
        assert!(matches!(
            decode_with_limits(&collection_bomb.0, limits),
            Err(WireDecodeError::CollectionLimitExceeded { maximum: 2 })
        ));
    }

    #[test]
    fn enforces_aggregate_string_budget_and_checked_length_arithmetic() {
        let mut frame = Writer::new(0, "abc", "def");
        frame.success_unit();
        let mut limits = DEFAULT_LIMITS;
        limits.max_total_string_bytes = 5;
        assert_eq!(
            decode_with_limits(&frame.0, limits),
            Err(WireDecodeError::StringBudgetExceeded { maximum: 5 })
        );

        let mut reader = Reader::new(&[], DEFAULT_LIMITS);
        reader.position = usize::MAX;
        assert_eq!(reader.read_exact(1), Err(WireDecodeError::LengthOverflow));

        reader.position = 0;
        reader.string_bytes = usize::MAX;
        reader.bytes = &[1, 0, 0, 0, b'a'];
        assert_eq!(reader.read_text(), Err(WireDecodeError::LengthOverflow));
    }

    #[test]
    fn rejects_noncanonical_boolean_and_nested_tags() {
        let mut boolean = Writer::new(0, "s", "f");
        boolean.u32(1);
        boolean.u32(2);
        boolean.u32(2);
        assert_eq!(
            decode(&boolean.0),
            Err(WireDecodeError::UnknownTag {
                context: "boolean",
                tag: 2
            })
        );

        let mut storage = Writer::new(1, "s", "f");
        storage.event_header(2, "f", &[]);
        storage.text("e");
        storage.u32(99);
        storage.success_unit();
        assert_eq!(
            decode(&storage.0),
            Err(WireDecodeError::UnknownTag {
                context: "storage",
                tag: 99
            })
        );
    }

    #[test]
    fn accepts_only_exact_compiler_owned_status_mappings() {
        for code in 1..=2 {
            decode(&select_failure_frame(
                "semaprax.status.v1",
                "semaprax.contract.v1",
                code,
                1,
                1,
            ))
            .unwrap();
        }
        for code in 1..=8 {
            decode(&select_failure_frame(
                "semaprax.status.v1",
                "semaprax.arithmetic.v1",
                code,
                2,
                1,
            ))
            .unwrap();
        }
    }

    #[test]
    fn rejects_malformed_status_v1_fields() {
        let cases = [
            (
                select_failure_frame("semaprax.status.v2", "semaprax.contract.v1", 1, 1, 1),
                "schema must be semaprax.status.v1",
            ),
            (
                select_failure_frame("semaprax.status.v1", "", 1, 1, 1),
                "domain identity cannot be empty",
            ),
            (
                select_failure_frame("semaprax.status.v1", "bad\0domain", 1, 1, 1),
                "domain identity cannot contain NUL",
            ),
            (
                select_failure_frame("semaprax.status.v1", "semaprax.contract.v1", 0, 1, 1),
                "status code zero is reserved for success",
            ),
        ];
        for (frame, reason) in cases {
            assert_eq!(decode(&frame), Err(WireDecodeError::InvalidStatus(reason)));
        }

        let long_domain = "x".repeat(256);
        assert_eq!(
            decode(&select_failure_frame(
                "semaprax.status.v1",
                &long_domain,
                1,
                1,
                1,
            )),
            Err(WireDecodeError::InvalidStatus(
                "domain identity cannot exceed 255 UTF-8 bytes"
            ))
        );
    }

    #[test]
    fn rejects_forged_compiler_status_mappings_and_external_classes() {
        let cases = [
            (
                "semaprax.arithmetic.v1",
                1,
                1,
                1,
                "contract class requires semaprax.contract.v1 and code 1 or 2",
            ),
            (
                "semaprax.contract.v1",
                1,
                2,
                1,
                "arithmetic class requires semaprax.arithmetic.v1 and a StatusCase code 1 through 8",
            ),
            (
                "semaprax.contract.v1",
                3,
                1,
                1,
                "contract class requires semaprax.contract.v1 and code 1 or 2",
            ),
            (
                "semaprax.arithmetic.v1",
                9,
                2,
                1,
                "arithmetic class requires semaprax.arithmetic.v1 and a StatusCase code 1 through 8",
            ),
            (
                "semaprax.contract.v1",
                1,
                1,
                0,
                "compiler-owned statuses must have retryability false",
            ),
            (
                "semaprax.arithmetic.v1",
                1,
                2,
                2,
                "compiler-owned statuses must have retryability false",
            ),
        ];
        for (domain, code, class, retryability, reason) in cases {
            assert_eq!(
                decode(&select_failure_frame(
                    "semaprax.status.v1",
                    domain,
                    code,
                    class,
                    retryability,
                )),
                Err(WireDecodeError::InvalidStatus(reason))
            );
        }

        for class in 3..=5 {
            assert_eq!(
                decode(&select_failure_frame(
                    "semaprax.status.v1",
                    "external.example",
                    1,
                    class,
                    1,
                )),
                Err(WireDecodeError::InvalidStatus(
                    "external status classes are outside the current native trace slice"
                ))
            );
        }
    }

    #[test]
    fn rejects_infeasible_counts_before_large_reserves() {
        let mut events = Writer::new(100, "s", "f");
        events.success_unit();
        let mut limits = DEFAULT_LIMITS;
        limits.max_events = 100;
        assert!(matches!(
            decode_with_limits(&events.0, limits),
            Err(WireDecodeError::Truncated { .. })
        ));

        let mut nested = Writer::new(1, "s", "f");
        nested.u32(2);
        nested.text("f");
        nested.u32(100);
        nested.success_unit();
        let mut limits = DEFAULT_LIMITS;
        limits.max_collection_items = 101;
        assert!(matches!(
            decode_with_limits(&nested.0, limits),
            Err(WireDecodeError::Truncated { .. })
        ));
    }
}
