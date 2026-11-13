//! Exact selected cache projection. Only current checked source grants authority.
use super::ValidatedPartitionProgram;
use crate::{ast::Program, cache_codec, graph};

const SCHEMA: &[u8] = b"semaprax.private-byte-partition-cache.v1\0";
const REVISION_BYTES: usize = "sha256:".len() + 64;
pub(super) const HEADER_BYTES: usize = SCHEMA.len() + REVISION_BYTES;

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "private partition has no ordinary cache route")
)]
impl<'source> ValidatedPartitionProgram<'source> {
    pub(crate) fn cache_projection(&self) -> cache_codec::Result<Vec<u8>> {
        let revision = graph::revision(self.source());
        cache_codec::encode_prefixed(
            self.program(),
            cache_codec::MAX_BYTES,
            &[SCHEMA, revision.as_bytes()],
        )
    }

    /// Replay the complete selected compiler path, then compare the exact
    /// current HIR projection. The supplied bytes never become backend HIR.
    pub(crate) fn replay_cache(
        source: &'source Program,
        wire: &[u8],
        work: &mut usize,
    ) -> cache_codec::Result<Self> {
        let result = (|| {
            if wire.len() > cache_codec::MAX_BYTES {
                return Err(cache_codec::capacity(
                    "private byte partition cache exceeds its bound",
                ));
            }
            let payload = wire
                .strip_prefix(SCHEMA)
                .filter(|_| wire.len() > HEADER_BYTES)
                .ok_or_else(|| {
                    cache_codec::grammar("private byte partition cache header differs")
                })?;
            let (revision, cached_hir) = payload.split_at(REVISION_BYTES);
            super::debit(work, wire.len()).map_err(|diagnostic| vec![diagnostic])?;
            if revision != graph::revision(source).as_bytes() {
                return Err(cache_codec::grammar(
                    "private byte partition cache source revision differs",
                ));
            }
            let current =
                Self::compile_source(source, work).map_err(|diagnostic| vec![diagnostic])?;
            let hir = cache_codec::encode_bounded(current.program(), cached_hir.len())?;
            if cached_hir != hir {
                return Err(cache_codec::grammar(
                    "private byte partition cached HIR differs from source replay",
                ));
            }
            Ok(current)
        })();
        if result.is_err() {
            *work = 0;
        }
        result
    }
}
