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
#[path = "native_conformance_wire/tests.rs"]
mod tests;
