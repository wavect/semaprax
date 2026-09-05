//! Capacity flows contributed by command and network host operations.
//!
//! The mapping is keyed on the closed operation enum so a new table entry
//! must be classified here explicitly; `hir::byte_capacity` mirrors every
//! allocation-relevant arm so source and HIR summaries agree.

use std::collections::BTreeMap;

use super::source_transcript_source_from_roots;
use crate::ast::Expr;
use crate::byte_data_capacity::{CapacityFlow, TranscriptSource};
use crate::hir::ResolvedHostCommandOperation;

pub(super) fn flow(
    operation: ResolvedHostCommandOperation,
    path: &str,
    args: &[Expr],
    transcript_roots: &BTreeMap<String, TranscriptSource>,
) -> Option<CapacityFlow> {
    let site = path.to_owned();
    match operation {
        ResolvedHostCommandOperation::StdinRead => Some(CapacityFlow::StdinRead {
            site,
            conservative_payload_bytes: crate::command_io_ops::MAX_INPUT_BYTES,
        }),
        ResolvedHostCommandOperation::StderrWrite => Some(CapacityFlow::StderrWrite {
            site,
            source: source_transcript_source_from_roots(args.first()?, transcript_roots),
        }),
        // `net_stream_stdout` publishes through the same runtime-bounded
        // stdout transcript as `stdout_append`.
        ResolvedHostCommandOperation::StdoutAppend
        | ResolvedHostCommandOperation::NetStreamStdout => {
            Some(CapacityFlow::StdoutAppend { site })
        }
        ResolvedHostCommandOperation::StderrAppend => Some(CapacityFlow::StderrAppend { site }),
        // One bounded network read is an owned-byte allocation site with the
        // conservative chunk payload; it is not a stdin read.
        ResolvedHostCommandOperation::NetRecv => Some(CapacityFlow::BytesCopy {
            site,
            conservative_payload_bytes: crate::network_io_ops::MAX_CHUNK_BYTES,
        }),
        ResolvedHostCommandOperation::HttpsGet => Some(CapacityFlow::BytesCopy {
            site,
            conservative_payload_bytes: crate::network_io_ops::MAX_CHUNK_BYTES,
        }),
        ResolvedHostCommandOperation::ArgsLen
        | ResolvedHostCommandOperation::ArgUtf8
        | ResolvedHostCommandOperation::NetConnect
        | ResolvedHostCommandOperation::NetSend
        | ResolvedHostCommandOperation::NetWait
        | ResolvedHostCommandOperation::NetClose
        | ResolvedHostCommandOperation::NetTlsConnect
        | ResolvedHostCommandOperation::NetListen
        | ResolvedHostCommandOperation::NetAccept
        | ResolvedHostCommandOperation::NetTlsAccept
        | ResolvedHostCommandOperation::NetCloseListener => None,
    }
}
