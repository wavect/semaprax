//! Independent ordinary-family facts. These are not selected primitive leases.
mod compare;
mod consuming;
mod debug_contains;
mod iterator;
mod iterator_shape;
mod leaf;
mod record_presence;
mod scratch;
pub(crate) use record_presence::RecordPresence;
pub(crate) mod sort;
mod validation;
mod vector;
use crate::hir::{ExpressionId, ResolvedBinding, ResolvedFunction, ResolvedProgram};
pub(crate) use scratch::{Admission, Refusal, Scratch};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Family {
    Iterator = 0,
    Vec = 1,
    ByteSet = 2,
    String = 3,
    Helper = 4,
}
impl Family {
    fn bit(self) -> u8 {
        1 << self as u8
    }
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Entry<'a> {
    at: &'a ExpressionId,
    binding: &'a ResolvedBinding,
    families: u8,
    family: Family,
    ordinal: usize,
    rank: u8,
    first: bool,
}
/// The phase retains the original immutable tree. Borrowed keys contain no
/// cloned identities, and exact subject pointers are checked on each receiver.
#[derive(Debug)]
pub(crate) struct Directory<'a> {
    program: &'a ResolvedProgram,
    function: &'a ResolvedFunction,
    entries: Vec<Entry<'a>>,
}
/// Only registered iterator/ordinal projections construct this token with exact
/// borrowed pointers, never source or cleanup-plan authority.
#[derive(Clone, Copy)]
pub(crate) struct CurrentMember<'a> {
    program: &'a ResolvedProgram,
    function: &'a ResolvedFunction,
}
pub(crate) fn current_members(
    program: &ResolvedProgram,
) -> impl Iterator<Item = CurrentMember<'_>> {
    program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .map(move |function| CurrentMember { program, function })
}
pub(crate) fn current_member_at(
    program: &ResolvedProgram,
    ordinal: usize,
) -> Option<CurrentMember<'_>> {
    let function = program.functions.get(ordinal).or_else(|| {
        let instance = ordinal.checked_sub(program.functions.len())?;
        program
            .function_instances
            .get(instance)
            .map(|i| &i.function)
    })?;
    Some(CurrentMember { program, function })
}
impl<'a> CurrentMember<'a> {
    pub(crate) fn function(&self) -> &'a ResolvedFunction {
        self.function
    }
}
impl<'a> Directory<'a> {
    /// Factory consumes independently checked helper sites from the exact
    /// phase index; every ordinary family is rebuilt from the current tree.
    #[cfg(test)]
    pub(crate) fn collect(
        program: &'a ResolvedProgram,
        function: &'a ResolvedFunction,
        helpers: &crate::loop_calls::byte_writer::ModuleIndex<'a>,
        admission: &mut Admission<'_>,
    ) -> Result<Self, Refusal> {
        admission.units(1)?;
        if !helpers.matches_program(program) {
            return admission.refuse(Refusal::Subject);
        }
        admission.reserve(RecordPresence::transport_controls())?;
        Self::collect_impl(program, function, helpers, None, admission, true)
    }
    #[cfg(test)]
    pub(crate) fn collect_current(
        member: CurrentMember<'a>,
        helpers: &crate::loop_calls::byte_writer::ModuleIndex<'a>,
        admission: &mut Admission<'_>,
    ) -> Result<Self, Refusal> {
        admission.units(1)?;
        if !helpers.matches_program(member.program) {
            return admission.refuse(Refusal::Subject);
        }
        admission.reserve(RecordPresence::transport_controls())?;
        Self::collect_impl(
            member.program,
            member.function,
            helpers,
            None,
            admission,
            false,
        )
    }
    #[cfg(test)]
    pub(crate) fn collect_current_with_presence(
        member: CurrentMember<'a>,
        helpers: &crate::loop_calls::byte_writer::ModuleIndex<'a>,
        presence: &RecordPresence<'_>,
        admission: &mut Admission<'_>,
    ) -> Result<Self, Refusal> {
        admission.units(1)?;
        if !helpers.matches_program(member.program) {
            return admission.refuse(Refusal::Subject);
        }
        Self::collect_impl(
            member.program,
            member.function,
            helpers,
            Some(presence),
            admission,
            false,
        )
    }
    /// Shared production route: no optional presence transport or match.
    /// The complete preceding optional factory remains an independent test arm.
    pub(crate) fn collect_current_shared(
        member: CurrentMember<'a>,
        helpers: &crate::loop_calls::byte_writer::ModuleIndex<'a>,
        presence: &RecordPresence<'_>,
        admission: &mut Admission<'_>,
    ) -> Result<Self, Refusal> {
        admission.units(1)?;
        if !helpers.matches_program(member.program) {
            return admission.refuse(Refusal::Subject);
        }
        let program = member.program;
        let function = member.function;
        Self::build_inner(program, function, admission, false, |output, admission| {
            iterator::collect(program, function, output, admission)?;
            vector::collect(program, function, output, admission)?;
            consuming::collect_with_presence(program, function, output, presence, admission)?;
            leaf::collect(function, Family::ByteSet, output, admission)?;
            leaf::collect(function, Family::String, output, admission)?;
            let count = admission.arithmetic(
                program
                    .functions
                    .len()
                    .checked_add(program.function_instances.len()),
            )?;
            let probes = if count == 0 {
                0
            } else {
                usize::BITS as usize - count.leading_zeros() as usize
            };
            admission.units(probes)?;
            if let Some(sites) = helpers.function_sites(function) {
                for (at, binding) in sites.borrowed_entries() {
                    admission.units(1)?;
                    Self::emit(output, Family::Helper, at, binding, admission)?;
                }
            }
            Ok(())
        })
    }

    #[cfg(test)]
    fn collect_impl(
        program: &'a ResolvedProgram,
        function: &'a ResolvedFunction,
        helpers: &crate::loop_calls::byte_writer::ModuleIndex<'a>,
        presence: Option<&RecordPresence<'_>>,
        admission: &mut Admission<'_>,
        check_member: bool,
    ) -> Result<Self, Refusal> {
        Self::build_inner(
            program,
            function,
            admission,
            check_member,
            |output, admission| {
                iterator::collect(program, function, output, admission)?;
                vector::collect(program, function, output, admission)?;
                match presence {
                    Some(presence) => consuming::collect_with_presence(
                        program, function, output, presence, admission,
                    )?,
                    None => consuming::collect(program, function, output, admission)?,
                }
                leaf::collect(function, Family::ByteSet, output, admission)?;
                leaf::collect(function, Family::String, output, admission)?;
                let count = admission.arithmetic(
                    program
                        .functions
                        .len()
                        .checked_add(program.function_instances.len()),
                )?;
                let probes = if count == 0 {
                    0
                } else {
                    usize::BITS as usize - count.leading_zeros() as usize
                };
                admission.units(probes)?;
                if let Some(sites) = helpers.function_sites(function) {
                    for (at, binding) in sites.borrowed_entries() {
                        admission.units(1)?;
                        Self::emit(output, Family::Helper, at, binding, admission)?;
                    }
                }
                Ok(())
            },
        )
    }

    #[cfg(test)]
    fn build(
        program: &'a ResolvedProgram,
        function: &'a ResolvedFunction,
        admission: &mut Admission<'_>,
        collect: impl FnOnce(&mut Scratch<Entry<'a>>, &mut Admission<'_>) -> Result<(), Refusal>,
    ) -> Result<Self, Refusal> {
        Self::build_inner(program, function, admission, true, collect)
    }
    fn build_inner(
        program: &'a ResolvedProgram,
        function: &'a ResolvedFunction,
        admission: &mut Admission<'_>,
        check_member: bool,
        collect: impl FnOnce(&mut Scratch<Entry<'a>>, &mut Admission<'_>) -> Result<(), Refusal>,
    ) -> Result<Self, Refusal> {
        if check_member {
            let mut member = false;
            for candidate in program
                .functions
                .iter()
                .chain(program.function_instances.iter().map(|i| &i.function))
            {
                admission.units(1)?;
                member |= std::ptr::eq(candidate, function);
            }
            if !member {
                return admission.refuse(Refusal::Subject);
            }
        }
        admission.reserve(std::mem::size_of::<Self>())?;
        let mut entries = Scratch::new(admission)?;
        collect(&mut entries, admission)?;
        let mut entries = entries.into_vec();
        sort::sort(&mut entries, admission)?;
        let mut output = 0usize;
        for input in 0..entries.len() {
            admission.units(1)?;
            let entry = entries[input];
            if output != 0
                && sort::compare_keys(entries[output - 1].at, entry.at, admission)?.is_eq()
            {
                let previous = &mut entries[output - 1];
                previous.families |= entry.families;
                // Sort uses family priority then original collector ordinal.
                previous.binding = entry.binding;
                previous.family = entry.family;
                previous.ordinal = entry.ordinal;
                previous.rank = entry.rank;
                previous.first = entry.first;
            } else {
                entries[output] = entry;
                output += 1;
            }
        }
        entries.truncate(output);
        Ok(Self {
            program,
            function,
            entries,
        })
    }
    fn emit(
        entries: &mut Scratch<Entry<'a>>,
        family: Family,
        at: &'a ExpressionId,
        binding: &'a ResolvedBinding,
        admission: &mut Admission<'_>,
    ) -> Result<(), Refusal> {
        entries.push(
            Entry {
                at,
                binding,
                families: family.bit(),
                family,
                ordinal: entries.as_slice().len(),
                rank: 1,
                first: false,
            },
            admission,
        )
    }
    fn emit_consuming(
        entries: &mut Scratch<Entry<'a>>,
        at: &'a ExpressionId,
        binding: &'a ResolvedBinding,
        admission: &mut Admission<'_>,
    ) -> Result<(), Refusal> {
        entries.push(
            Entry {
                at,
                binding,
                families: Family::Vec.bit(),
                family: Family::Vec,
                ordinal: entries.as_slice().len(),
                rank: 0,
                first: true,
            },
            admission,
        )
    }
    pub(crate) fn receive(
        &self,
        program: &ResolvedProgram,
        function: &ResolvedFunction,
        at: &ExpressionId,
        admission: &mut Admission<'_>,
    ) -> Result<Option<(&'a ResolvedBinding, u8)>, Refusal> {
        admission.reserve(0)?;
        admission.units(1)?;
        if !std::ptr::eq(self.program, program) || !std::ptr::eq(self.function, function) {
            return admission.refuse(Refusal::Subject);
        }
        let (mut begin, mut end) = (0, self.entries.len());
        while begin < end {
            let middle = begin + (end - begin) / 2;
            match sort::compare_keys(self.entries[middle].at, at, admission)? {
                std::cmp::Ordering::Less => begin = middle + 1,
                std::cmp::Ordering::Greater => end = middle,
                std::cmp::Ordering::Equal => {
                    return Ok(Some((
                        self.entries[middle].binding,
                        self.entries[middle].families,
                    )))
                }
            }
        }
        Ok(None)
    }
    pub(crate) fn function_key(&self) -> usize {
        self.function as *const _ as usize
    }
    pub(crate) fn backing_bytes(&self) -> usize {
        self.entries.capacity() * std::mem::size_of::<Entry<'_>>()
    }
    pub(crate) fn has_family(
        &self,
        family: Family,
        admission: &mut Admission<'_>,
    ) -> Result<bool, Refusal> {
        admission.reserve(0)?;
        for entry in &self.entries {
            admission.units(1)?;
            if Self::contains(entry.families, family) {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub(crate) fn contains(families: u8, family: Family) -> bool {
        families & family.bit() != 0
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod direct_presence_tests;
