//! Canonical byte preparation only; the sole wire validator owns admission.
use super::{wire, DoctorOfflineArchitecture, DoctorOfflineBundleError as Error};
use crate::DOCTOR_OFFLINE_INPUT_MAX_BYTES;

/// Explicit borrowed content. Paths must already be in canonical inventory
/// order; encoding never reads a host path or follows an alias to obtain bytes.
#[derive(Clone, Copy, Debug)]
pub struct DoctorOfflineBundleEntry<'a> {
    pub path: &'a str,
    pub bytes: &'a [u8],
    pub executable: bool,
}

/// Exact indices into the supplied, already sorted entry slice. Absent roles
/// are not inferred from filenames; at least one role must be explicitly set.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DoctorOfflineBundleRoles {
    pub clang: Option<usize>,
    pub node: Option<usize>,
    pub rustc: Option<usize>,
}

/// Encode one bounded canonical inventory without acquiring any authority.
/// Both named architectures can be serialized on any host; this does not admit
/// that host for execution or bypass subsequent sealed-input acquisition.
///
/// The caller may lower, never raise, the 512 MiB encoded-byte ceiling. Basic
/// scalar/storage bounds are checked before reserving the output. The existing
/// complete validator then checks the emitted bytes, including paths, roles,
/// minimum ELF and interpreter structure. Malformed input may therefore incur
/// one bounded output allocation before rejection; no partial bytes escape.
pub fn encode_doctor_offline_bundle(
    architecture: DoctorOfflineArchitecture,
    selector: &str,
    entries: &[DoctorOfflineBundleEntry<'_>],
    roles: DoctorOfflineBundleRoles,
    max_bytes: usize,
) -> Result<Vec<u8>, Error> {
    let plan = prepare(selector, entries, roles, max_bytes)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(plan.length)
        .map_err(|_| Error::Allocation)?;
    output.extend_from_slice(wire::MAGIC);
    output.push(match architecture {
        DoctorOfflineArchitecture::LinuxX86_64 => 1,
        DoctorOfflineArchitecture::LinuxAarch64 => 2,
    });
    output.push(plan.role_mask);
    output.extend_from_slice(&plan.selector_length.to_le_bytes());
    output.extend_from_slice(&plan.file_count.to_le_bytes());
    for index in plan.indices {
        output.extend_from_slice(&index.to_le_bytes());
    }
    output.extend_from_slice(selector.as_bytes());
    for entry in entries {
        // prepare checked every narrowing conversion before output reservation.
        let path_length = u16::try_from(entry.path.len()).map_err(|_| Error::Limit)?;
        let content_length = u64::try_from(entry.bytes.len()).map_err(|_| Error::Limit)?;
        output.extend_from_slice(&path_length.to_le_bytes());
        output.extend_from_slice(&[u8::from(entry.executable), 0]);
        output.extend_from_slice(&content_length.to_le_bytes());
        output.extend_from_slice(entry.path.as_bytes());
        output.extend_from_slice(entry.bytes);
    }
    if output.len() != plan.length {
        return Err(Error::Invalid);
    }
    // This is the existing complete validator, not a second inventory/ELF
    // implementation or an opaque sealed-input/bundle constructor.
    let _ = wire::parse(&output, selector, architecture)?;
    Ok(output)
}

struct Plan {
    length: usize,
    selector_length: u16,
    file_count: u32,
    role_mask: u8,
    indices: [u32; 3],
}

fn prepare(
    selector: &str,
    entries: &[DoctorOfflineBundleEntry<'_>],
    roles: DoctorOfflineBundleRoles,
    max_bytes: usize,
) -> Result<Plan, Error> {
    // Limit validation precedes inspection of the selector, records or roles.
    if max_bytes == 0 {
        return Err(Error::Invalid);
    }
    if max_bytes > DOCTOR_OFFLINE_INPUT_MAX_BYTES {
        return Err(Error::Limit);
    }
    if !wire::valid_selector(selector) || entries.is_empty() {
        return Err(Error::Invalid);
    }
    if entries.len() > wire::MAX_FILES {
        return Err(Error::Limit);
    }
    let selector_length = u16::try_from(selector.len()).map_err(|_| Error::Limit)?;
    let file_count = u32::try_from(entries.len()).map_err(|_| Error::Limit)?;
    let mut indices = [wire::ABSENT; 3];
    let mut role_mask = 0;
    for (role, index) in [roles.clang, roles.node, roles.rustc]
        .into_iter()
        .enumerate()
    {
        if let Some(index) = index {
            if index >= entries.len() {
                return Err(Error::Invalid);
            }
            indices[role] = u32::try_from(index).map_err(|_| Error::Limit)?;
            role_mask |= 1 << role;
        }
    }
    if role_mask == 0 {
        return Err(Error::Invalid);
    }
    let mut length = 28usize.checked_add(selector.len()).ok_or(Error::Limit)?;
    if length > max_bytes {
        return Err(Error::Limit);
    }
    let mut path_bytes = 0;
    for entry in entries {
        account_record(
            &mut length,
            &mut path_bytes,
            entry.path.len(),
            entry.bytes.len(),
            max_bytes,
        )?;
    }
    Ok(Plan {
        length,
        selector_length,
        file_count,
        role_mask,
        indices,
    })
}

// Length-only preflight is also exercised without allocating near-ceiling
// payloads. Such arithmetic scripts are not physical allocation evidence.
fn account_record(
    total: &mut usize,
    paths: &mut usize,
    path: usize,
    content: usize,
    max_bytes: usize,
) -> Result<(), Error> {
    if path == 0 {
        return Err(Error::Invalid);
    }
    if path > wire::MAX_PATH_BYTES || content > DOCTOR_OFFLINE_INPUT_MAX_BYTES {
        return Err(Error::Limit);
    }
    u16::try_from(path).map_err(|_| Error::Limit)?;
    u64::try_from(content).map_err(|_| Error::Limit)?;
    let next_paths = paths.checked_add(path).ok_or(Error::Limit)?;
    if next_paths > wire::MAX_TOTAL_PATH_BYTES {
        return Err(Error::Limit);
    }
    let next_total = total
        .checked_add(12)
        .and_then(|value| value.checked_add(path))
        .and_then(|value| value.checked_add(content))
        .ok_or(Error::Limit)?;
    if next_total > max_bytes {
        return Err(Error::Limit);
    }
    *paths = next_paths;
    *total = next_total;
    Ok(())
}

#[cfg(test)]
mod tests;
