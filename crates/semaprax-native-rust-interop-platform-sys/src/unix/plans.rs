//! Exact preflight plans for Unix commands, arenas, names, and inventories.
//!
//! Every entry point here allocates its owned capacity before any syscall so
//! that later held-handle phases stay allocation free.

use super::*;

pub(super) fn prepare_command(
    values: &[&str],
    output_capacity: usize,
) -> Result<PreparedCommand, Error> {
    let mut arguments = Vec::with_capacity(values.len());
    if arguments.capacity() != values.len() {
        return Err(Error::OutputLimit);
    }
    for value in values {
        arguments.push(argument(value)?);
    }
    let output = Vec::with_capacity(output_capacity);
    if output.capacity() != output_capacity {
        return Err(Error::OutputLimit);
    }
    Ok(PreparedCommand { arguments, output })
}

pub(super) fn prepared_command_owned_capacity(command: &PreparedCommand) -> usize {
    command
        .arguments
        .capacity()
        .saturating_mul(std::mem::size_of::<CString>())
        .saturating_add(
            command
                .arguments
                .iter()
                .map(|value| value.as_bytes_with_nul().len())
                .sum::<usize>(),
        )
        .saturating_add(command.output.capacity())
}

pub fn prepare_tool_resolver(
    fallback: &str,
    maximum: usize,
) -> Result<PreparedToolResolver, Error> {
    if maximum == 0
        || maximum > 32_768
        || fallback.is_empty()
        || fallback.as_bytes().contains(&b'/')
    {
        return Err(Error::Invalid);
    }
    let candidate = Vec::with_capacity(maximum);
    let canonical = Vec::with_capacity(maximum);
    let display = String::with_capacity(maximum);
    if candidate.capacity() != maximum
        || canonical.capacity() != maximum
        || display.capacity() != maximum
    {
        return Err(Error::OutputLimit);
    }
    Ok(PreparedToolResolver {
        candidate,
        canonical,
        display,
        fallback: CString::new(fallback).map_err(|_| Error::Invalid)?,
        maximum,
    })
}

pub fn prepared_tool_resolver_owned_capacity(prepared: &PreparedToolResolver) -> usize {
    prepared
        .candidate
        .capacity()
        .saturating_add(prepared.canonical.capacity())
        .saturating_add(prepared.display.capacity())
        .saturating_add(prepared.fallback.as_bytes_with_nul().len())
}

pub fn prepare_version_invocation(
    argument: &str,
    maximum: usize,
) -> Result<PreparedVersionInvocation, Error> {
    if maximum > 65_536 {
        return Err(Error::OutputLimit);
    }
    let argument = CString::new(argument).map_err(|_| Error::Invalid)?;
    let output = Vec::with_capacity(maximum);
    if output.capacity() != maximum {
        return Err(Error::OutputLimit);
    }
    Ok(PreparedVersionInvocation { argument, output })
}

pub fn prepare_sysroot_invocation(maximum: usize) -> Result<PreparedSysrootInvocation, Error> {
    prepare_version_invocation("--print=sysroot", maximum).map(PreparedSysrootInvocation)
}

pub fn prepare_rustc_version_invocation(
    maximum: usize,
) -> Result<PreparedRustcVersionInvocation, Error> {
    prepare_version_invocation("-vV", maximum).map(PreparedRustcVersionInvocation)
}

pub fn prepared_sysroot_owned_capacity(prepared: &PreparedSysrootInvocation) -> usize {
    prepared_version_owned_capacity(&prepared.0)
}

pub fn prepared_rustc_version_owned_capacity(prepared: &PreparedRustcVersionInvocation) -> usize {
    prepared_version_owned_capacity(&prepared.0)
}

pub fn prepared_version_owned_capacity(prepared: &PreparedVersionInvocation) -> usize {
    prepared
        .argument
        .as_bytes_with_nul()
        .len()
        .saturating_add(prepared.output.capacity())
}

pub fn prepare_process_arena_plan(uses: usize) -> Result<PreparedProcessArenaPlan, Error> {
    if uses == 0 || uses > 32 {
        return Err(Error::Invalid);
    }
    Ok(PreparedProcessArenaPlan { uses })
}

pub fn prepare_process_arena_plan_with_environment(
    uses: usize,
    include: Option<&OsStr>,
    libraries: Option<&OsStr>,
) -> Result<PreparedProcessArenaPlan, Error> {
    if include.is_some() || libraries.is_some() {
        return Err(Error::Invalid);
    }
    prepare_process_arena_plan(uses)
}

pub fn prepared_process_arena_plan_capacity(_: &PreparedProcessArenaPlan) -> usize {
    0
}

pub fn materialize_process_arena(
    plan: PreparedProcessArenaPlan,
) -> Result<PreparedProcessArena, Error> {
    Ok(PreparedProcessArena {
        remaining: plan.uses,
    })
}

pub fn materialize_process_arena_with_environment(
    plan: PreparedProcessArenaPlan,
    include: Option<&OsStr>,
    libraries: Option<&OsStr>,
) -> Result<PreparedProcessArena, Error> {
    if include.is_some() || libraries.is_some() {
        return Err(Error::Invalid);
    }
    materialize_process_arena(plan)
}

pub fn prepare_process_arena(uses: usize) -> Result<PreparedProcessArena, Error> {
    materialize_process_arena(prepare_process_arena_plan(uses)?)
}

pub fn prepared_process_arena_owned_capacity(_: &PreparedProcessArena) -> usize {
    0
}

pub fn prepared_process_arena_remaining(prepared: &PreparedProcessArena) -> usize {
    prepared.remaining
}

pub(crate) fn consume_process_arena(prepared: &mut PreparedProcessArena) -> Result<(), Error> {
    prepared.remaining = prepared
        .remaining
        .checked_sub(1)
        .ok_or(Error::OutputLimit)?;
    Ok(())
}

pub(super) fn prepared_name_bindings<const N: usize>(
    names: &PreparedDiscardNames<N>,
) -> Result<[(usize, usize); N], Error> {
    let mut bindings = [(0, 0); N];
    for (index, binding) in bindings.iter_mut().enumerate() {
        let name = prepared_discard_name(names, index)?;
        *binding = (name.0.as_ptr() as usize, name.0.as_bytes_with_nul().len());
    }
    Ok(bindings)
}

pub fn inventory_exact_required_capacity<const N: usize>(
    names: &PreparedDiscardNames<N>,
) -> Result<usize, Error> {
    let mut total = 0usize;
    for index in 0..N {
        total = total
            .checked_add(
                prepared_discard_name(names, index)?
                    .0
                    .as_bytes_with_nul()
                    .len(),
            )
            .ok_or(Error::OutputLimit)?;
    }
    total
        .checked_add(
            INVENTORY_EXACT_ARENA_WORDS
                .checked_mul(std::mem::size_of::<u64>())
                .ok_or(Error::OutputLimit)?,
        )
        .ok_or(Error::OutputLimit)
}

pub fn prepare_inventory_exact<const N: usize>(
    names: &PreparedDiscardNames<N>,
) -> Result<PreparedInventoryExact<N>, Error> {
    let bindings = prepared_name_bindings(names)?;
    let mut copied = [const { None }; N];
    for (index, slot) in copied.iter_mut().enumerate() {
        let source = prepared_discard_name(names, index)?;
        let exact = source.0.as_bytes_with_nul().len();
        let mut bytes = Vec::with_capacity(exact);
        bytes.extend_from_slice(source.0.as_bytes_with_nul());
        if bytes.capacity() != exact {
            return Err(Error::OutputLimit);
        }
        *slot = Some(
            CString::from_vec_with_nul(bytes)
                .map(PreparedRelativeName)
                .map_err(|_| Error::Invalid)?,
        );
    }
    Ok(PreparedInventoryExact {
        names: copied,
        bindings,
        storage: vec![0_u64; INVENTORY_EXACT_ARENA_WORDS].into_boxed_slice(),
        directory_identity: None,
        remaining: 2,
        #[cfg(test)]
        scan_entries: 0,
        #[cfg(test)]
        fail_initial_seek: false,
        #[cfg(test)]
        fail_reset_seek: false,
        #[cfg(test)]
        fail_rebound_authentication: false,
        #[cfg(test)]
        fail_rebound_close: false,
    })
}

pub fn prepared_inventory_exact_owned_capacity<const N: usize>(
    prepared: &PreparedInventoryExact<N>,
) -> usize {
    prepared
        .names
        .iter()
        .filter_map(Option::as_ref)
        .map(|name| name.0.as_bytes_with_nul().len())
        .sum::<usize>()
        .saturating_add(
            prepared
                .storage
                .len()
                .saturating_mul(std::mem::size_of::<u64>()),
        )
}

pub fn prepared_inventory_exact_remaining<const N: usize>(
    prepared: &PreparedInventoryExact<N>,
) -> u8 {
    prepared.remaining
}

pub fn prepare_inventory_entries_exact<const N: usize>(
    names: [&OsStr; N],
    file_count: usize,
) -> Result<PreparedInventoryEntriesExact<N>, Error> {
    if N == 0 || file_count > N {
        return Err(Error::Invalid);
    }
    Ok(PreparedInventoryEntriesExact {
        names: prepare_discard_names(names)?,
        file_count,
        storage: vec![0_u64; INVENTORY_EXACT_ARENA_WORDS].into_boxed_slice(),
        remaining: 1,
    })
}

pub fn prepared_inventory_entries_exact_owned_capacity<const N: usize>(
    prepared: &PreparedInventoryEntriesExact<N>,
) -> usize {
    prepared_discard_names_owned_capacity(&prepared.names).saturating_add(
        prepared
            .storage
            .len()
            .saturating_mul(std::mem::size_of::<u64>()),
    )
}

pub fn publish_directory_required_capacity(name: &OsStr) -> Result<usize, Error> {
    validated_c_name_bytes(name)?
        .len()
        .checked_add(1)
        .ok_or(Error::OutputLimit)
}

pub fn prepare_publish_directory(name: &OsStr) -> Result<PreparedPublishDirectory, Error> {
    let bytes = validated_c_name_bytes(name)?;
    let exact_capacity = bytes.len().checked_add(1).ok_or(Error::OutputLimit)?;
    let mut copied = Vec::with_capacity(exact_capacity);
    copied.extend_from_slice(bytes);
    copied.push(0);
    if copied.capacity() != exact_capacity {
        return Err(Error::OutputLimit);
    }
    let destination = CString::from_vec_with_nul(copied).map_err(|_| Error::Invalid)?;
    Ok(PreparedPublishDirectory {
        destination,
        exact_capacity,
        remaining: 1,
        #[cfg(debug_assertions)]
        fail_before_open: false,
        #[cfg(debug_assertions)]
        fail_information: false,
        #[cfg(debug_assertions)]
        fail_close: false,
        #[cfg(debug_assertions)]
        fail_rename: false,
    })
}

pub fn prepared_publish_directory_owned_capacity(prepared: &PreparedPublishDirectory) -> usize {
    prepared.destination.as_bytes_with_nul().len()
}

pub fn prepared_publish_directory_remaining(prepared: &PreparedPublishDirectory) -> u8 {
    prepared.remaining
}

#[cfg(debug_assertions)]
pub fn inject_publish_directory_failure(
    prepared: &mut PreparedPublishDirectory,
    point: u8,
) -> Result<(), Error> {
    match point {
        1 => prepared.fail_before_open = true,
        2 => prepared.fail_information = true,
        3 => prepared.fail_close = true,
        4 => prepared.fail_rename = true,
        _ => return Err(Error::Invalid),
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn test_inventory_exact_failures<const N: usize>(
    prepared: &mut PreparedInventoryExact<N>,
    initial_seek: bool,
    reset_seek: bool,
    rebound_authentication: bool,
    rebound_close: bool,
) {
    prepared.fail_initial_seek = initial_seek;
    prepared.fail_reset_seek = reset_seek;
    prepared.fail_rebound_authentication = rebound_authentication;
    prepared.fail_rebound_close = rebound_close;
}

#[cfg(test)]
pub(crate) fn test_inventory_exact_scan_entries<const N: usize>(
    prepared: &PreparedInventoryExact<N>,
) -> usize {
    prepared.scan_entries
}

pub fn prepare_link_or_copy<const N: usize>(
    names: &PreparedDiscardNames<N>,
    destination_index: usize,
) -> Result<PreparedLinkOrCopy, Error> {
    let destination = prepared_discard_name(names, destination_index)?;
    let exact = destination.0.as_bytes_with_nul().len();
    let mut bytes = Vec::with_capacity(exact);
    bytes.extend_from_slice(destination.0.as_bytes_with_nul());
    if bytes.capacity() != exact {
        return Err(Error::OutputLimit);
    }
    let destination = CString::from_vec_with_nul(bytes)
        .map(PreparedRelativeName)
        .map_err(|_| Error::Invalid)?;
    Ok(PreparedLinkOrCopy {
        destination_index,
        destination,
        #[cfg(debug_assertions)]
        fail_before_authentication: false,
    })
}

pub fn link_or_copy_required_capacity<const N: usize>(
    names: &PreparedDiscardNames<N>,
    destination_index: usize,
) -> Result<usize, Error> {
    Ok(prepared_discard_name(names, destination_index)?
        .0
        .as_bytes_with_nul()
        .len())
}

pub fn prepared_link_or_copy_owned_capacity(prepared: &PreparedLinkOrCopy) -> usize {
    prepared.destination.0.as_bytes_with_nul().len()
}

#[cfg(debug_assertions)]
pub fn inject_link_or_copy_failure_before_authentication(prepared: &mut PreparedLinkOrCopy) {
    prepared.fail_before_authentication = true;
}

pub fn prepare_relative_name(name: &OsStr) -> Result<PreparedRelativeName, Error> {
    let bytes = name.as_bytes();
    if bytes.is_empty()
        || bytes == b"."
        || bytes == b".."
        || bytes.contains(&b'/')
        || bytes.contains(&0)
    {
        return Err(Error::Invalid);
    }
    let exact = bytes.len().checked_add(1).ok_or(Error::OutputLimit)?;
    let mut owned = Vec::with_capacity(exact);
    owned.extend_from_slice(bytes);
    owned.push(0);
    if owned.capacity() != exact {
        return Err(Error::OutputLimit);
    }
    CString::from_vec_with_nul(owned)
        .map(PreparedRelativeName)
        .map_err(|_| Error::Invalid)
}

pub fn prepare_relative_name_arena(maximum: usize) -> Result<PreparedRelativeNameArena, Error> {
    let capacity = maximum.checked_add(1).ok_or(Error::OutputLimit)?;
    let bytes = Vec::with_capacity(capacity);
    if bytes.capacity() != capacity {
        return Err(Error::OutputLimit);
    }
    Ok(PreparedRelativeNameArena { bytes, maximum })
}

pub fn set_relative_name_arena(
    arena: &mut PreparedRelativeNameArena,
    name: &OsStr,
) -> Result<(), Error> {
    let bytes = name.as_bytes();
    if bytes.is_empty()
        || bytes.len() > arena.maximum
        || bytes == b"."
        || bytes == b".."
        || bytes.contains(&b'/')
        || bytes.contains(&0)
    {
        return Err(Error::Invalid);
    }
    let capacity = arena.maximum.checked_add(1).ok_or(Error::OutputLimit)?;
    arena.bytes.clear();
    arena.bytes.extend_from_slice(bytes);
    arena.bytes.push(0);
    if arena.bytes.capacity() != capacity {
        return Err(Error::OutputLimit);
    }
    Ok(())
}

pub fn relative_name_arena_capacity(arena: &PreparedRelativeNameArena) -> usize {
    arena.bytes.capacity()
}

pub(super) fn relative_name_arena_cstr(
    arena: &PreparedRelativeNameArena,
) -> Result<&std::ffi::CStr, Error> {
    std::ffi::CStr::from_bytes_with_nul(&arena.bytes).map_err(|_| Error::Invalid)
}

pub fn prepare_discard_names<const N: usize>(
    names: [&OsStr; N],
) -> Result<PreparedDiscardNames<N>, Error> {
    let names = names.map(|name| prepare_relative_name(name).ok());
    if names.iter().any(Option::is_none) {
        return Err(Error::Invalid);
    }
    for left in 0..N {
        for right in 0..left {
            if names[left].as_ref().expect("validated").0.as_bytes()
                == names[right].as_ref().expect("validated").0.as_bytes()
            {
                return Err(Error::Invalid);
            }
        }
    }
    Ok(PreparedDiscardNames { names })
}

pub fn prepared_discard_names_owned_capacity<const N: usize>(
    prepared: &PreparedDiscardNames<N>,
) -> usize {
    prepared
        .names
        .iter()
        .filter_map(Option::as_ref)
        .map(|name| name.0.as_bytes_with_nul().len())
        .sum()
}

pub(super) fn prepared_discard_name<const N: usize>(
    prepared: &PreparedDiscardNames<N>,
    index: usize,
) -> Result<&PreparedRelativeName, Error> {
    prepared
        .names
        .get(index)
        .and_then(Option::as_ref)
        .ok_or(Error::Invalid)
}

#[cfg(target_os = "macos")]
pub(super) fn metadata_generation(metadata: &std::fs::Metadata) -> u32 {
    use std::os::macos::fs::MetadataExt as _;
    metadata.st_gen()
}
