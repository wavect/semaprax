//! Closed one-shot binding data. Neither requests nor replies confer authority.
use super::super::{
    DoctorOfflineArchitecture, DoctorOfflineTool, ProbeError, DOCTOR_OFFLINE_INPUT_MAX_BYTES,
};
use super::Error;
use sha2::{Digest, Sha256};

pub(super) const MAX_REQUEST_BYTES: usize = 149;
pub(super) const MAX_REPLY_BYTES: usize = 3 * 65_536 + 128;
const REQUEST_MAGIC: &[u8; 8] = b"SPXDWK1\0";
const REPLY_MAGIC: &[u8; 8] = b"SPXDWR1\0";
const REPLY_HEADER: usize = 77;

#[derive(Debug)]
pub(super) struct Request {
    pub(super) nonce: [u8; 32],
    pub(super) digest: [u8; 32],
    pub(super) bundle_digest: [u8; 32],
    pub(super) bundle_len: usize,
    pub(super) architecture: DoctorOfflineArchitecture,
    pub(super) target: u8,
    pub(super) roles: u8,
    pub(super) selector: String,
}

impl Request {
    pub(super) fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_REQUEST_BYTES {
            return Err(Error::Limit);
        }
        let mut cursor = 0;
        if take(bytes, &mut cursor, 8)? != REQUEST_MAGIC || byte(bytes, &mut cursor)? != 1 {
            return Err(Error::Invalid);
        }
        let architecture = match byte(bytes, &mut cursor)? {
            1 => DoctorOfflineArchitecture::LinuxX86_64,
            2 => DoctorOfflineArchitecture::LinuxAarch64,
            _ => return Err(Error::Invalid),
        };
        let target = byte(bytes, &mut cursor)?;
        let expected_roles = match target {
            0 => 4,
            1 => 1,
            2 => 2,
            3 => 7,
            _ => return Err(Error::Invalid),
        };
        let roles = byte(bytes, &mut cursor)?;
        if roles != expected_roles {
            return Err(Error::Invalid);
        }
        let nonce = array(bytes, &mut cursor)?;
        if nonce == [0; 32] {
            return Err(Error::Invalid);
        }
        let length = u64::from_le_bytes(array(bytes, &mut cursor)?);
        let bundle_len = usize::try_from(length).map_err(|_| Error::Limit)?;
        if bundle_len == 0 {
            return Err(Error::Invalid);
        }
        if bundle_len > DOCTOR_OFFLINE_INPUT_MAX_BYTES {
            return Err(Error::Limit);
        }
        let bundle_digest = array(bytes, &mut cursor)?;
        let selector_len = usize::from(byte(bytes, &mut cursor)?);
        if selector_len > 64 {
            return Err(Error::Limit);
        }
        let selector_bytes = take(bytes, &mut cursor, selector_len)?;
        if cursor != bytes.len()
            || selector_bytes.is_empty()
            || !selector_bytes[0].is_ascii_lowercase()
            || !selector_bytes
                .iter()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        {
            return Err(Error::Invalid);
        }
        let mut selector = String::new();
        selector
            .try_reserve_exact(selector_len)
            .map_err(|_| Error::Allocation)?;
        selector.push_str(std::str::from_utf8(selector_bytes).map_err(|_| Error::Invalid)?);
        Ok(Self {
            nonce,
            digest: Sha256::digest(bytes).into(),
            bundle_digest,
            bundle_len,
            architecture,
            target,
            roles,
            selector,
        })
    }

    pub(super) fn roles(&self) -> impl Iterator<Item = (u8, DoctorOfflineTool)> + '_ {
        [
            (1, DoctorOfflineTool::Clang),
            (2, DoctorOfflineTool::Node),
            (4, DoctorOfflineTool::Rustc),
        ]
        .into_iter()
        .filter(|(role, _)| self.roles & role != 0)
    }

    fn platform(&self) -> [u8; 4] {
        let architecture = match self.architecture {
            DoctorOfflineArchitecture::LinuxX86_64 => 1,
            DoctorOfflineArchitecture::LinuxAarch64 => 2,
        };
        [1, architecture, self.target, self.roles]
    }
}

pub(super) type ReplyRow = (u8, Result<Vec<u8>, ProbeError>);

pub(super) fn encode_reply(request: &Request, rows: &[ReplyRow]) -> Result<Vec<u8>, Error> {
    if rows.len() != request.roles().count() {
        return Err(Error::Invalid);
    }
    let mut length = REPLY_HEADER;
    for ((role, value), (expected, _)) in rows.iter().zip(request.roles()) {
        if *role != expected {
            return Err(Error::Invalid);
        }
        let payload_len = value.as_ref().map_or(0, Vec::len);
        if payload_len > 65_536 {
            return Err(Error::Limit);
        }
        length = length
            .checked_add(6 + payload_len)
            .filter(|length| *length <= MAX_REPLY_BYTES)
            .ok_or(Error::Limit)?;
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| Error::Allocation)?;
    output.extend_from_slice(REPLY_MAGIC);
    output.extend_from_slice(&request.digest);
    output.extend_from_slice(&request.nonce);
    output.extend_from_slice(&request.platform());
    output.push(u8::try_from(rows.len()).map_err(|_| Error::Limit)?);
    for (role, value) in rows {
        output.push(*role);
        match value {
            Ok(payload) => {
                output.push(0);
                output.extend_from_slice(&(payload.len() as u32).to_le_bytes());
                output.extend_from_slice(payload);
            }
            Err(error) => {
                output.push(encode_error(*error));
                output.extend_from_slice(&0u32.to_le_bytes());
            }
        }
    }
    Ok(output)
}

/// Validate only byte binding and shape. The collector separately owns the live
/// worker, endpoint, successful termination and descendant-settlement proof.
pub(super) fn validate_reply(request: &Request, bytes: &[u8]) -> Result<Vec<ReplyRow>, Error> {
    if bytes.len() > MAX_REPLY_BYTES {
        return Err(Error::Limit);
    }
    let mut cursor = 0;
    if take(bytes, &mut cursor, 8)? != REPLY_MAGIC
        || take(bytes, &mut cursor, 32)? != request.digest
        || take(bytes, &mut cursor, 32)? != request.nonce
        || take(bytes, &mut cursor, 4)? != request.platform()
        || usize::from(byte(bytes, &mut cursor)?) != request.roles().count()
    {
        return Err(Error::Invalid);
    }
    // Validate the complete frame before allocating any returned payload.
    let rows_start = cursor;
    for (expected_role, _) in request.roles() {
        if byte(bytes, &mut cursor)? != expected_role {
            return Err(Error::Invalid);
        }
        let status = byte(bytes, &mut cursor)?;
        let length = usize::try_from(u32::from_le_bytes(array(bytes, &mut cursor)?))
            .map_err(|_| Error::Limit)?;
        if length > 65_536 {
            return Err(Error::Limit);
        }
        if status > 7 || (status != 0 && length != 0) {
            return Err(Error::Invalid);
        }
        take(bytes, &mut cursor, length)?;
    }
    if cursor != bytes.len() {
        return Err(Error::Invalid);
    }
    let mut rows = Vec::new();
    rows.try_reserve_exact(request.roles().count())
        .map_err(|_| Error::Allocation)?;
    cursor = rows_start;
    for _ in request.roles() {
        let role = byte(bytes, &mut cursor)?;
        let status = byte(bytes, &mut cursor)?;
        let length = usize::try_from(u32::from_le_bytes(array(bytes, &mut cursor)?))
            .map_err(|_| Error::Limit)?;
        let payload = take(bytes, &mut cursor, length)?;
        let value = if status == 0 {
            let mut output = Vec::new();
            output
                .try_reserve_exact(length)
                .map_err(|_| Error::Allocation)?;
            output.extend_from_slice(payload);
            Ok(output)
        } else {
            Err(decode_error(status)?)
        };
        rows.push((role, value));
    }
    Ok(rows)
}

fn encode_error(error: ProbeError) -> u8 {
    match error {
        ProbeError::Invalid => 1,
        ProbeError::Unsupported => 2,
        ProbeError::Spawn => 3,
        ProbeError::Exit => 4,
        ProbeError::OutputLimit => 5,
        ProbeError::Timeout => 6,
        ProbeError::Io => 7,
    }
}

fn decode_error(status: u8) -> Result<ProbeError, Error> {
    match status {
        1 => Ok(ProbeError::Invalid),
        2 => Ok(ProbeError::Unsupported),
        3 => Ok(ProbeError::Spawn),
        4 => Ok(ProbeError::Exit),
        5 => Ok(ProbeError::OutputLimit),
        6 => Ok(ProbeError::Timeout),
        7 => Ok(ProbeError::Io),
        _ => Err(Error::Invalid),
    }
}

fn take<'a>(bytes: &'a [u8], cursor: &mut usize, length: usize) -> Result<&'a [u8], Error> {
    let end = cursor.checked_add(length).ok_or(Error::Invalid)?;
    let slice = bytes.get(*cursor..end).ok_or(Error::Invalid)?;
    *cursor = end;
    Ok(slice)
}

fn byte(bytes: &[u8], cursor: &mut usize) -> Result<u8, Error> {
    Ok(take(bytes, cursor, 1)?[0])
}

fn array<const N: usize>(bytes: &[u8], cursor: &mut usize) -> Result<[u8; N], Error> {
    take(bytes, cursor, N)?
        .try_into()
        .map_err(|_| Error::Invalid)
}

#[cfg(test)]
mod tests;
