//! Exact preflight plans for Windows commands, arenas, wide names, and
//! inventories, plus the relative-open and identity primitives they use.

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
        arguments.push((*value).to_owned());
    }
    let command_line = windows_command_line(&arguments)?;
    let output = Vec::with_capacity(output_capacity);
    if output.capacity() != output_capacity {
        return Err(Error::OutputLimit);
    }
    Ok(PreparedCommand {
        arguments,
        command_line,
        output,
    })
}

pub(super) fn prepared_command_owned_capacity(command: &PreparedCommand) -> usize {
    command
        .arguments
        .capacity()
        .saturating_mul(std::mem::size_of::<String>())
        .saturating_add(
            command
                .arguments
                .iter()
                .map(String::capacity)
                .sum::<usize>(),
        )
        .saturating_add(
            command
                .command_line
                .capacity()
                .saturating_mul(std::mem::size_of::<u16>()),
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
        || fallback.contains(['/', '\\', '\0'])
    {
        return Err(Error::Invalid);
    }
    let candidate = Vec::with_capacity(maximum);
    let canonical = Vec::with_capacity(maximum);
    let display_capacity = maximum.checked_mul(3).ok_or(Error::OutputLimit)?;
    let display = String::with_capacity(display_capacity);
    let fallback = fallback.encode_utf16().collect::<Vec<_>>();
    if candidate.capacity() != maximum
        || canonical.capacity() != maximum
        || display.capacity() != display_capacity
    {
        return Err(Error::OutputLimit);
    }
    Ok(PreparedToolResolver {
        candidate,
        canonical,
        display,
        fallback,
        maximum,
    })
}

pub fn prepared_tool_resolver_owned_capacity(prepared: &PreparedToolResolver) -> usize {
    prepared
        .candidate
        .capacity()
        .saturating_add(prepared.canonical.capacity())
        .saturating_add(prepared.fallback.capacity())
        .saturating_mul(std::mem::size_of::<u16>())
        .saturating_add(prepared.display.capacity())
}

pub fn prepare_version_invocation(
    argument: &str,
    maximum: usize,
) -> Result<PreparedVersionInvocation, Error> {
    if maximum > 65_536 {
        return Err(Error::OutputLimit);
    }
    let command_line = windows_command_line(&[argument.to_owned()])?;
    let output = Vec::with_capacity(maximum);
    if output.capacity() != maximum {
        return Err(Error::OutputLimit);
    }
    Ok(PreparedVersionInvocation {
        command_line,
        output,
    })
}

pub fn prepared_version_owned_capacity(prepared: &PreparedVersionInvocation) -> usize {
    prepared
        .command_line
        .capacity()
        .saturating_mul(std::mem::size_of::<u16>())
        .saturating_add(prepared.output.capacity())
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

pub(crate) fn process_arena_plan(
    uses: usize,
    attribute_bytes: usize,
    environment_units: usize,
) -> Result<PreparedProcessArenaPlan, Error> {
    if uses == 0 || uses > 32 {
        return Err(Error::Invalid);
    }
    if attribute_bytes == 0 {
        return Err(Error::Unsupported);
    }
    if attribute_bytes > MAX_PROCESS_ATTRIBUTE_BYTES {
        return Err(Error::OutputLimit);
    }
    if !(2..=MAX_PROCESS_ENVIRONMENT_UNITS).contains(&environment_units) {
        return Err(Error::OutputLimit);
    }
    let attribute_words = attribute_bytes
        .checked_add(std::mem::size_of::<u64>() - 1)
        .and_then(|bytes| bytes.checked_div(std::mem::size_of::<u64>()))
        .ok_or(Error::OutputLimit)?;
    let path_capacity = PROCESS_PATH_UNITS
        .checked_mul(std::mem::size_of::<u16>())
        .and_then(|bytes| bytes.checked_mul(2))
        .ok_or(Error::OutputLimit)?;
    let owned_capacity = attribute_words
        .checked_mul(std::mem::size_of::<u64>())
        .and_then(|bytes| path_capacity.checked_add(bytes))
        .and_then(|bytes| {
            environment_units
                .checked_mul(std::mem::size_of::<u16>())
                .and_then(|environment| bytes.checked_add(environment))
        })
        .ok_or(Error::OutputLimit)?;
    Ok(PreparedProcessArenaPlan {
        uses,
        attribute_bytes,
        attribute_words,
        environment_units,
        owned_capacity,
    })
}

fn process_environment_units(
    include: Option<&OsStr>,
    libraries: Option<&OsStr>,
) -> Result<usize, Error> {
    match (include, libraries) {
        (None, None) => Ok(2),
        (Some(include), Some(libraries)) => {
            let include_units = include.encode_wide().try_fold(0usize, |count, unit| {
                if unit == 0 {
                    Err(Error::Invalid)
                } else {
                    count.checked_add(1).ok_or(Error::OutputLimit)
                }
            })?;
            let library_units = libraries.encode_wide().try_fold(0usize, |count, unit| {
                if unit == 0 {
                    Err(Error::Invalid)
                } else {
                    count.checked_add(1).ok_or(Error::OutputLimit)
                }
            })?;
            6usize
                .checked_add(include_units)
                .and_then(|units| units.checked_add(1))
                .and_then(|units| units.checked_add(8))
                .and_then(|units| units.checked_add(include_units))
                .and_then(|units| units.checked_add(1))
                .and_then(|units| units.checked_add(4))
                .and_then(|units| units.checked_add(library_units))
                .and_then(|units| units.checked_add(2))
                .filter(|units| *units <= MAX_PROCESS_ENVIRONMENT_UNITS)
                .ok_or(Error::OutputLimit)
        }
        _ => Err(Error::Invalid),
    }
}

pub fn prepare_process_arena_plan(uses: usize) -> Result<PreparedProcessArenaPlan, Error> {
    if uses == 0 || uses > 32 {
        return Err(Error::Invalid);
    }
    let mut attribute_bytes = 0_usize;
    let initialized = unsafe {
        InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut attribute_bytes)
    };
    if initialized != 0 || unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER {
        return Err(Error::Unsupported);
    }
    process_arena_plan(uses, attribute_bytes, 2)
}

pub fn prepare_process_arena_plan_with_environment(
    uses: usize,
    include: Option<&OsStr>,
    libraries: Option<&OsStr>,
) -> Result<PreparedProcessArenaPlan, Error> {
    let environment_units = process_environment_units(include, libraries)?;
    let mut attribute_bytes = 0_usize;
    let initialized = unsafe {
        InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut attribute_bytes)
    };
    if initialized != 0 || unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER {
        return Err(Error::Unsupported);
    }
    process_arena_plan(uses, attribute_bytes, environment_units)
}

pub fn prepared_process_arena_plan_capacity(plan: &PreparedProcessArenaPlan) -> usize {
    plan.owned_capacity
}

pub fn materialize_process_arena(
    plan: PreparedProcessArenaPlan,
) -> Result<PreparedProcessArena, Error> {
    materialize_process_arena_with_environment(plan, None, None)
}

pub fn materialize_process_arena_with_environment(
    plan: PreparedProcessArenaPlan,
    include: Option<&OsStr>,
    libraries: Option<&OsStr>,
) -> Result<PreparedProcessArena, Error> {
    if process_environment_units(include, libraries)? != plan.environment_units {
        return Err(Error::Invalid);
    }
    let application = Vec::with_capacity(PROCESS_PATH_UNITS);
    let cwd = Vec::with_capacity(PROCESS_PATH_UNITS);
    let mut environment = Vec::with_capacity(plan.environment_units);
    match (include, libraries) {
        (None, None) => environment.extend([0, 0]),
        (Some(include), Some(libraries)) => {
            // CI and the product invoke Clang's GNU-compatible driver, which
            // consumes CPATH rather than clang-cl's INCLUDE convention. Both
            // names carry the same already-bounded caller-selected directory
            // set; this adds no SDK discovery or ambient PATH authority.
            environment.extend("CPATH=".encode_utf16());
            environment.extend(include.encode_wide());
            environment.push(0);
            environment.extend("INCLUDE=".encode_utf16());
            environment.extend(include.encode_wide());
            environment.push(0);
            environment.extend("LIB=".encode_utf16());
            environment.extend(libraries.encode_wide());
            environment.extend([0, 0]);
        }
        _ => return Err(Error::Invalid),
    }
    let attributes = Vec::with_capacity(plan.attribute_words);
    if application.capacity() != PROCESS_PATH_UNITS
        || cwd.capacity() != PROCESS_PATH_UNITS
        || environment.capacity() != plan.environment_units
        || environment.len() != plan.environment_units
        || attributes.capacity() != plan.attribute_words
    {
        return Err(Error::OutputLimit);
    }
    Ok(PreparedProcessArena {
        application,
        cwd,
        environment,
        attributes,
        attribute_bytes: plan.attribute_bytes,
        remaining: plan.uses,
    })
}

pub fn prepare_process_arena(uses: usize) -> Result<PreparedProcessArena, Error> {
    materialize_process_arena(prepare_process_arena_plan(uses)?)
}

pub fn prepared_process_arena_owned_capacity(prepared: &PreparedProcessArena) -> usize {
    prepared
        .application
        .capacity()
        .saturating_mul(std::mem::size_of::<u16>())
        .saturating_add(
            prepared
                .environment
                .capacity()
                .saturating_mul(std::mem::size_of::<u16>()),
        )
        .saturating_add(
            prepared
                .cwd
                .capacity()
                .saturating_mul(std::mem::size_of::<u16>()),
        )
        .saturating_add(
            prepared
                .attributes
                .capacity()
                .saturating_mul(std::mem::size_of::<u64>()),
        )
}

#[cfg(test)]
pub(crate) fn test_prepared_process_environment(prepared: &PreparedProcessArena) -> &[u16] {
    &prepared.environment
}

pub fn prepared_process_arena_remaining(prepared: &PreparedProcessArena) -> usize {
    prepared.remaining
}

pub(crate) fn consume_process_arena(prepared: &mut PreparedProcessArena) -> Result<(), Error> {
    let attribute_words = prepared
        .attribute_bytes
        .checked_add(std::mem::size_of::<u64>() - 1)
        .and_then(|bytes| bytes.checked_div(std::mem::size_of::<u64>()))
        .ok_or(Error::OutputLimit)?;
    if prepared.application.capacity() != PROCESS_PATH_UNITS
        || prepared.cwd.capacity() != PROCESS_PATH_UNITS
        || !(2..=MAX_PROCESS_ENVIRONMENT_UNITS).contains(&prepared.environment.capacity())
        || prepared.environment.len() != prepared.environment.capacity()
        || !prepared.environment.ends_with(&[0, 0])
        || prepared.attributes.capacity() != attribute_words
    {
        return Err(Error::OutputLimit);
    }
    prepared.remaining = prepared
        .remaining
        .checked_sub(1)
        .ok_or(Error::OutputLimit)?;
    prepared.application.clear();
    prepared.cwd.clear();
    prepared.attributes.clear();
    Ok(())
}

pub(super) fn prepared_name_bindings<const N: usize>(
    names: &PreparedDiscardNames<N>,
) -> Result<[(usize, usize); N], Error> {
    let mut bindings = [(0, 0); N];
    for (index, binding) in bindings.iter_mut().enumerate() {
        let name = prepared_discard_name(names, index)?;
        *binding = (name.0.as_ptr() as usize, name.0.len());
    }
    Ok(bindings)
}

pub fn inventory_exact_required_capacity<const N: usize>(
    names: &PreparedDiscardNames<N>,
) -> Result<usize, Error> {
    let names = (0..N).try_fold(0usize, |total, index| {
        total
            .checked_add(
                prepared_discard_name(names, index)?
                    .0
                    .len()
                    .checked_mul(std::mem::size_of::<u16>())
                    .ok_or(Error::OutputLimit)?,
            )
            .ok_or(Error::OutputLimit)
    })?;
    names
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
        let mut units = Vec::with_capacity(source.0.len());
        units.extend_from_slice(&source.0);
        if units.capacity() != source.0.len() {
            return Err(Error::OutputLimit);
        }
        *slot = Some(PreparedRelativeName(units));
    }
    let storage = vec![0_u64; INVENTORY_EXACT_ARENA_WORDS].into_boxed_slice();
    Ok(PreparedInventoryExact {
        names: copied,
        bindings,
        storage,
        directory_identity: None,
        remaining: 2,
    })
}

pub fn prepared_inventory_exact_owned_capacity<const N: usize>(
    prepared: &PreparedInventoryExact<N>,
) -> usize {
    prepared
        .names
        .iter()
        .filter_map(Option::as_ref)
        .map(|name| name.0.capacity().saturating_mul(std::mem::size_of::<u16>()))
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

fn publish_information_layout(name_units: usize) -> Result<(usize, usize), Error> {
    let name_bytes = name_units.checked_mul(2).ok_or(Error::OutputLimit)?;
    let total = std::mem::size_of::<NamedInformation>()
        .checked_add(name_bytes)
        .ok_or(Error::OutputLimit)?;
    let words = total
        .checked_add(std::mem::size_of::<usize>() - 1)
        .ok_or(Error::OutputLimit)?
        / std::mem::size_of::<usize>();
    Ok((total, words))
}

pub fn publish_directory_required_capacity(name: &OsStr) -> Result<usize, Error> {
    let text = prepared_normal_name(name)?;
    let (_, words) = publish_information_layout(text.encode_utf16().count())?;
    words
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or(Error::OutputLimit)
}

pub fn prepare_publish_directory(name: &OsStr) -> Result<PreparedPublishDirectory, Error> {
    let text = prepared_normal_name(name)?;
    let name_units = text.encode_utf16().count();
    let (total, words) = publish_information_layout(name_units)?;
    let exact_capacity = words
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or(Error::OutputLimit)?;
    let mut storage = vec![0_usize; words].into_boxed_slice();
    let information = storage.as_mut_ptr().cast::<NamedInformation>();
    unsafe {
        (*information).flags = 0;
        (*information).root_directory = std::ptr::null_mut();
        (*information).file_name_length =
            u32::try_from(name_units.checked_mul(2).ok_or(Error::OutputLimit)?)
                .map_err(|_| Error::OutputLimit)?;
        let destination = std::ptr::addr_of_mut!((*information).file_name).cast::<u16>();
        for (index, unit) in text.encode_utf16().enumerate() {
            destination.add(index).write(unit);
        }
    }
    Ok(PreparedPublishDirectory {
        storage,
        total,
        name_units,
        exact_capacity,
        remaining: 1,
        #[cfg(test)]
        force_extended_rejection: false,
        #[cfg(test)]
        observed_extended_flags: None,
        #[cfg(test)]
        observed_legacy_flags: None,
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
    prepared
        .storage
        .len()
        .saturating_mul(std::mem::size_of::<usize>())
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

pub fn prepare_link_or_copy<const N: usize>(
    names: &PreparedDiscardNames<N>,
    destination_index: usize,
) -> Result<PreparedLinkOrCopy, Error> {
    let name = prepared_discard_name(names, destination_index)?;
    let (total, words) = link_information_layout(name)?;
    let name_bytes = name.0.len().checked_mul(2).ok_or(Error::OutputLimit)?;
    let mut storage = vec![0_usize; words].into_boxed_slice();
    let information = storage.as_mut_ptr().cast::<NamedInformation>();
    unsafe {
        (*information).flags = 0;
        (*information).root_directory = std::ptr::null_mut();
        (*information).file_name_length =
            u32::try_from(name_bytes).map_err(|_| Error::OutputLimit)?;
        std::ptr::copy_nonoverlapping(
            name.0.as_ptr(),
            std::ptr::addr_of_mut!((*information).file_name).cast::<u16>(),
            name.0.len(),
        );
    }
    Ok(PreparedLinkOrCopy {
        destination_index,
        storage,
        total,
        #[cfg(debug_assertions)]
        fail_before_authentication: false,
    })
}

fn link_information_layout(name: &PreparedRelativeName) -> Result<(usize, usize), Error> {
    let name_bytes = name.0.len().checked_mul(2).ok_or(Error::OutputLimit)?;
    let fixed = std::mem::offset_of!(NamedInformation, file_name);
    let total = fixed
        .checked_add(name_bytes)
        .ok_or(Error::OutputLimit)?
        .max(std::mem::size_of::<NamedInformation>());
    let words = total
        .checked_add(std::mem::size_of::<usize>() - 1)
        .ok_or(Error::OutputLimit)?
        / std::mem::size_of::<usize>();
    Ok((total, words))
}

pub fn link_or_copy_required_capacity<const N: usize>(
    names: &PreparedDiscardNames<N>,
    destination_index: usize,
) -> Result<usize, Error> {
    let name = prepared_discard_name(names, destination_index)?;
    let (_, words) = link_information_layout(name)?;
    words
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or(Error::OutputLimit)
}

pub fn prepared_link_or_copy_owned_capacity(prepared: &PreparedLinkOrCopy) -> usize {
    prepared
        .storage
        .len()
        .saturating_mul(std::mem::size_of::<usize>())
}

#[cfg(debug_assertions)]
pub fn inject_link_or_copy_failure_before_authentication(prepared: &mut PreparedLinkOrCopy) {
    prepared.fail_before_authentication = true;
}

fn ascii_fold(value: u16) -> u16 {
    if value >= u16::from(b'a') && value <= u16::from(b'z') {
        value - u16::from(b'a' - b'A')
    } else {
        value
    }
}

fn prepared_names_equal(left: &PreparedRelativeName, right: &PreparedRelativeName) -> bool {
    left.0.len() == right.0.len()
        && left
            .0
            .iter()
            .zip(&right.0)
            .all(|(left, right)| ascii_fold(*left) == ascii_fold(*right))
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

pub(super) fn prepared_matches_slice(expected: &PreparedRelativeName, actual: &[u16]) -> bool {
    expected.0.len() == actual.len()
        && expected
            .0
            .iter()
            .zip(actual)
            .all(|(expected, actual)| ascii_fold(*expected) == ascii_fold(*actual))
}

pub(super) fn prepared_normal_name(name: &OsStr) -> Result<&str, Error> {
    let text = name.to_str().ok_or(Error::Invalid)?;
    if text.is_empty()
        || !text.is_ascii()
        || matches!(text, "." | "..")
        || text.contains(['/', '\\', '\0'])
        || text.ends_with([' ', '.'])
        || text.contains(':')
    {
        return Err(Error::Invalid);
    }
    let stem = text.split('.').next().ok_or(Error::Invalid)?;
    if ["CON", "PRN", "AUX", "NUL", "CLOCK$"]
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
        || (stem.len() == 4
            && (stem[..3].eq_ignore_ascii_case("COM") || stem[..3].eq_ignore_ascii_case("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    {
        return Err(Error::Invalid);
    }
    Ok(text)
}

pub fn prepare_relative_name(name: &OsStr) -> Result<PreparedRelativeName, Error> {
    let text = prepared_normal_name(name)?;
    let exact = text.encode_utf16().count();
    let mut encoded = Vec::with_capacity(exact);
    encoded.extend(text.encode_utf16());
    if encoded.len() != exact || encoded.capacity() != exact {
        return Err(Error::OutputLimit);
    }
    Ok(PreparedRelativeName(encoded))
}

pub fn prepare_relative_name_arena(maximum: usize) -> Result<PreparedRelativeNameArena, Error> {
    let units = Vec::with_capacity(maximum);
    if units.capacity() != maximum {
        return Err(Error::OutputLimit);
    }
    Ok(PreparedRelativeNameArena { units, maximum })
}

pub fn set_relative_name_arena(
    arena: &mut PreparedRelativeNameArena,
    name: &OsStr,
) -> Result<(), Error> {
    let text = prepared_normal_name(name)?;
    if text.encode_utf16().count() > arena.maximum {
        return Err(Error::OutputLimit);
    }
    arena.units.clear();
    arena.units.extend(text.encode_utf16());
    if arena.units.capacity() != arena.maximum {
        return Err(Error::OutputLimit);
    }
    Ok(())
}

pub fn relative_name_arena_capacity(arena: &PreparedRelativeNameArena) -> usize {
    arena.units.capacity()
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
            if prepared_names_equal(
                names[left].as_ref().expect("validated"),
                names[right].as_ref().expect("validated"),
            ) {
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
        .map(|name| name.0.capacity().saturating_mul(std::mem::size_of::<u16>()))
        .sum()
}

pub(super) fn normal_name(name: &OsStr) -> Result<(), Error> {
    let text = name.to_str().ok_or(Error::Invalid)?;
    if text.is_empty()
        || !text.is_ascii()
        || matches!(text, "." | "..")
        || text.contains(['/', '\\', '\0'])
        || text.ends_with([' ', '.'])
        || text.contains(':')
    {
        return Err(Error::Invalid);
    }
    let stem = text
        .split('.')
        .next()
        .ok_or(Error::Invalid)?
        .to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
        || stem.strip_prefix("COM").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
        || stem.strip_prefix("LPT").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
    {
        return Err(Error::Invalid);
    }
    Ok(())
}

pub(super) fn open_directory(path: &Path) -> Result<File, Error> {
    open_absolute(path, DIRECTORY_READ_ACCESS, DIRECTORY_FLAGS)
}

pub(super) fn open_absolute(path: &Path, access: u32, flags: u32) -> Result<File, Error> {
    let path = wide_null(path.as_os_str())?;
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            access,
            HELD_SHARE,
            std::ptr::null(),
            OPEN_EXISTING,
            flags,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(Error::Changed);
    }
    Ok(unsafe { File::from_raw_handle(handle.cast()) })
}

pub(super) fn relative_file(
    parent: &File,
    name: &OsStr,
    access: u32,
    disposition: u32,
    create_options: u32,
) -> Result<File, Error> {
    let name = prepare_relative_name(name)?;
    relative_file_prepared(parent, &name, access, disposition, create_options)
}

pub(super) fn relative_file_prepared(
    parent: &File,
    name: &PreparedRelativeName,
    access: u32,
    disposition: u32,
    create_options: u32,
) -> Result<File, Error> {
    relative_file_units(parent, &name.0, access, disposition, create_options)
}

pub(super) fn relative_file_arena(
    parent: &File,
    name: &PreparedRelativeNameArena,
    access: u32,
    disposition: u32,
    create_options: u32,
) -> Result<File, Error> {
    relative_file_units(parent, &name.units, access, disposition, create_options)
}

pub(super) fn relative_file_units(
    parent: &File,
    name: &[u16],
    access: u32,
    disposition: u32,
    create_options: u32,
) -> Result<File, Error> {
    let byte_length = name.len().checked_mul(2).ok_or(Error::Invalid)?;
    let length = u16::try_from(byte_length).map_err(|_| Error::Invalid)?;
    let unicode = UNICODE_STRING {
        Length: length,
        MaximumLength: length,
        Buffer: name.as_ptr().cast_mut(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: u32::try_from(std::mem::size_of::<OBJECT_ATTRIBUTES>())
            .map_err(|_| Error::Changed)?,
        RootDirectory: parent.as_raw_handle().cast(),
        ObjectName: &unicode,
        Attributes: OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: std::ptr::null(),
        SecurityQualityOfService: std::ptr::null(),
    };
    let mut io = IO_STATUS_BLOCK::default();
    let mut handle = std::ptr::null_mut();
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            access,
            &attributes,
            &mut io,
            std::ptr::null(),
            FILE_ATTRIBUTE_NORMAL,
            HELD_SHARE,
            disposition,
            create_options | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
            std::ptr::null(),
            0,
        )
    };
    if status < 0 {
        return Err(if status == STATUS_OBJECT_NAME_COLLISION {
            Error::Exists
        } else {
            Error::Changed
        });
    }
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return Err(Error::Changed);
    }
    Ok(unsafe { File::from_raw_handle(handle.cast()) })
}

pub(super) fn open_relative_regular_read(parent: &Directory, name: &OsStr) -> Result<File, Error> {
    relative_file(
        &parent.file,
        name,
        REGULAR_READ_ACCESS,
        FILE_OPEN,
        FILE_NON_DIRECTORY_FILE,
    )
}

pub(super) fn information(file: &File) -> Result<Identity, Error> {
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe {
        GetFileInformationByHandle(
            file.as_raw_handle().cast::<core::ffi::c_void>(),
            &mut information,
        )
    } == 0
    {
        return Err(Error::Changed);
    }
    let mut file_id = FILE_ID_INFO::default();
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle().cast::<core::ffi::c_void>(),
            FileIdInfo,
            (&mut file_id as *mut FILE_ID_INFO).cast(),
            u32::try_from(std::mem::size_of::<FILE_ID_INFO>()).map_err(|_| Error::Changed)?,
        )
    } == 0
    {
        return Err(Error::Changed);
    }
    Ok(Identity {
        volume: file_id.VolumeSerialNumber,
        file_id: file_id.FileId.Identifier,
        attributes: information.dwFileAttributes,
        length: (u64::from(information.nFileSizeHigh) << 32) | u64::from(information.nFileSizeLow),
    })
}

fn stable_directory_identity(identity: Identity) -> Result<DirectoryIdentity, Error> {
    let stable_attributes =
        identity.attributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT);
    if stable_attributes != FILE_ATTRIBUTE_DIRECTORY {
        return Err(Error::Changed);
    }
    Ok(DirectoryIdentity {
        volume: identity.volume,
        file_id: identity.file_id,
        stable_attributes,
    })
}

pub(super) fn directory_information(file: &File) -> Result<DirectoryIdentity, Error> {
    let identity = stable_directory_identity(information(file)?)?;
    if !file.metadata().map_err(|_| Error::Changed)?.is_dir() {
        return Err(Error::Changed);
    }
    Ok(identity)
}

pub(super) fn digest(file: &File, length: u64) -> Result<[u8; 32], Error> {
    let mut file = file.try_clone().map_err(|_| Error::Changed)?;
    file.seek(SeekFrom::Start(0)).map_err(|_| Error::Changed)?;
    let mut hasher = Sha256::new();
    let mut remaining = length;
    let mut buffer = [0_u8; 8192];
    while remaining != 0 {
        let maximum =
            usize::try_from(remaining.min(buffer.len() as u64)).map_err(|_| Error::OutputLimit)?;
        let count = file
            .read(&mut buffer[..maximum])
            .map_err(|_| Error::Changed)?;
        if count == 0 {
            return Err(Error::Changed);
        }
        hasher.update(&buffer[..count]);
        remaining -= u64::try_from(count).map_err(|_| Error::OutputLimit)?;
    }
    Ok(hasher.finalize().into())
}

pub(super) fn digest_bytes(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

pub(super) fn final_path_prepared(file: &File, output: &mut Vec<u16>) -> Result<(), Error> {
    let maximum = output.capacity();
    if maximum != PROCESS_PATH_UNITS {
        return Err(Error::OutputLimit);
    }
    output.clear();
    output.resize(maximum, 0);
    let written = unsafe {
        GetFinalPathNameByHandleW(
            file.as_raw_handle().cast(),
            output.as_mut_ptr(),
            u32::try_from(maximum).map_err(|_| Error::OutputLimit)?,
            0,
        )
    };
    let written = usize::try_from(written).map_err(|_| Error::Changed)?;
    if written == 0 || written >= maximum {
        return Err(Error::OutputLimit);
    }
    output.truncate(written);
    let prefix = [
        u16::from(b'\\'),
        u16::from(b'\\'),
        u16::from(b'?'),
        u16::from(b'\\'),
    ];
    if output.starts_with(&prefix) {
        output.copy_within(prefix.len().., 0);
        output.truncate(written - prefix.len());
    }
    output.push(0);
    if output.capacity() != maximum {
        return Err(Error::OutputLimit);
    }
    Ok(())
}
