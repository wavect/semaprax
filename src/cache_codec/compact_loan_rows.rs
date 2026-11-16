//! Experimentally pack only fixed loan row fields, preserving all proof values.
use crate::cache_codec::{capacity, grammar, Codec, Decoder, Encoder, Result};
use crate::loan_plan::{LoanEdge, LoanEndpoint, LoanId, MAX_LOANS_PER_FUNCTION_V1};

fn count(length: usize) -> Result<u16> {
    if length > MAX_LOANS_PER_FUNCTION_V1 {
        return Err(capacity(
            "compact loan set exceeds its existing function limit",
        ));
    }
    u16::try_from(length).map_err(|_| capacity("compact loan set length exceeds u16"))
}

fn encode_ids(out: &mut Encoder, ids: &[LoanId]) -> Result<()> {
    for id in ids {
        id.0.encode(out)?;
    }
    Ok(())
}

fn decode_ids(input: &mut Decoder<'_>, length: u16) -> Result<Box<[LoanId]>> {
    let length = usize::from(length);
    count(length)?;
    input.sequence(length, std::mem::size_of::<LoanId>())?;
    let mut ids = Vec::new();
    ids.try_reserve_exact(length)
        .map_err(|_| capacity("compact loan set allocation failed"))?;
    if ids.capacity() != length {
        return Err(capacity(
            "compact loan set allocation differs from its exact carrier",
        ));
    }
    for _ in 0..length {
        ids.push(LoanId(u16::decode(input)?));
    }
    Ok(ids.into_boxed_slice())
}

pub(super) struct CompactEndpoint;
impl CompactEndpoint {
    pub(super) fn encode(row: &LoanEndpoint, out: &mut Encoder) -> Result<()> {
        out.nested(|out| {
            row.point.encode(out)?;
            let sets = [
                &*row.live_before,
                &*row.starts,
                &*row.kills,
                &*row.live_after,
            ];
            let mut lengths = 0u64;
            for (index, set) in sets.iter().enumerate() {
                lengths |= u64::from(count(set.len())?) << (16 * index);
            }
            lengths.encode(out)?;
            for set in sets {
                encode_ids(out, set)?;
            }
            Ok(())
        })
    }

    pub(super) fn decode(input: &mut Decoder<'_>) -> Result<LoanEndpoint> {
        input.nested(|input| {
            let point = Codec::decode(input)?;
            let lengths = u64::decode(input)?;
            Ok(LoanEndpoint {
                point,
                live_before: decode_ids(input, lengths as u16)?,
                starts: decode_ids(input, (lengths >> 16) as u16)?,
                kills: decode_ids(input, (lengths >> 32) as u16)?,
                live_after: decode_ids(input, (lengths >> 48) as u16)?,
            })
        })
    }
}

pub(super) struct CompactEdge;
impl CompactEdge {
    pub(super) fn encode(row: &LoanEdge, out: &mut Encoder) -> Result<()> {
        out.nested(|out| {
            let length = count(row.live.len())?;
            let packed =
                u64::from(row.from) | (u64::from(row.to) << 16) | (u64::from(length) << 32);
            packed.encode(out)?;
            encode_ids(out, &row.live)
        })
    }

    pub(super) fn decode(input: &mut Decoder<'_>) -> Result<LoanEdge> {
        input.nested(|input| {
            let packed = u64::decode(input)?;
            if packed >> 48 != 0 {
                return Err(grammar("compact loan edge has nonzero reserved bits"));
            }
            Ok(LoanEdge {
                from: packed as u16,
                to: (packed >> 16) as u16,
                live: decode_ids(input, (packed >> 32) as u16)?,
            })
        })
    }
}
