//! The ordinary directory is a separate current-tree proof, not HelperBindings.
mod numeric;
use super::*;
use crate::cleanup_plan::renewal_profile::ordinary::{
    Admission, Directory, Family, RecordPresence, Refusal, Scratch,
};
use crate::hir::ExpressionId;
impl<'a> ModuleIndex<'a> {
    pub(crate) fn new(program: &'a ResolvedProgram) -> Result<Self, Diagnostic> {
        let mut index = Self::new_helpers(program)?;
        // Continue the existing helper lookup account, including on the
        // allocation-free no-helper route. No second allowance is created.
        let remaining = &index.lookup_work;
        let failed = &index.ordinary_failed;
        let mut work = |units| Self::ordinary_units(remaining, failed, units);
        index.ordinary = index.collect_ordinary(program, None, &mut work)?;
        index.ordinary_positions = numeric::positions(&index.ordinary, &mut work)?;
        Ok(index)
    }
    /// Replay independently rebuilds demanded ordinary directories while its
    /// caller debits the existing replay ledger. Helper discovery is unchanged.
    pub(crate) fn new_with_ordinary_work(
        program: &'a ResolvedProgram,
        demanded: Option<&ResolvedFunction>,
        work: &mut dyn FnMut(usize) -> bool,
    ) -> Result<Self, Diagnostic> {
        let mut index = Self::new_helpers(program)?;
        index.ordinary = index.collect_ordinary(program, demanded, work)?;
        index.ordinary_positions = numeric::positions(&index.ordinary, work)?;
        Ok(index)
    }
    fn ordinary_units(
        remaining: &std::cell::Cell<usize>,
        failed: &std::cell::Cell<bool>,
        units: usize,
    ) -> bool {
        if failed.get() {
            return false;
        }
        match remaining.get().checked_sub(units) {
            Some(left) => {
                remaining.set(left);
                true
            }
            None => {
                failed.set(true);
                false
            }
        }
    }
    fn collect_ordinary(
        &self,
        program: &'a ResolvedProgram,
        demanded: Option<&ResolvedFunction>,
        work: &mut dyn FnMut(usize) -> bool,
    ) -> Result<Vec<Directory<'a>>, Diagnostic> {
        // Declare the borrowed inventory before Admission so its backing
        // remains alive until after the last borrowing receiver is dropped.
        #[allow(clippy::needless_late_init)]
        let probes;
        let mut reserve = crate::bounded_output::reserve_active_required;
        let mut admission = Admission::new(work, &mut reserve);
        probes =
            crate::hir::renewal_probes::phase::Probes::new(&program.declarations, &mut admission)
                .map_err(|_| resolved::census_error())?;
        admission.probes = Some(&probes);
        let record_presence = RecordPresence::new(&program.declarations, &mut admission)
            .map_err(|_| resolved::census_error())?;
        if self.sites.is_empty() {
            admission
                .reserve(std::mem::size_of::<Self>())
                .map_err(|_| resolved::census_error())?;
        }
        let mut directories = Scratch::new(&mut admission).map_err(|_| resolved::census_error())?;
        for member in crate::cleanup_plan::renewal_profile::ordinary::current_members(program) {
            let function = member.function();
            admission.units(1).map_err(|_| resolved::census_error())?;
            if demanded.is_some_and(|demanded| !std::ptr::eq(function, demanded)) {
                continue;
            }
            let directory = Directory::collect_current_with_presence(
                member,
                self,
                &record_presence,
                &mut admission,
            )
            .map_err(|_| resolved::census_error())?;
            directories
                .push(directory, &mut admission)
                .map_err(|_| resolved::census_error())?;
        }
        let directories = directories.into_vec();
        if demanded.is_some() && directories.len() != 1 {
            return Err(resolved::census_error());
        }
        Ok(directories)
    }
    fn ordinary_directory(
        &self,
        program: &ResolvedProgram,
        function: &ResolvedFunction,
        admission: &mut Admission<'_>,
    ) -> Result<&Directory<'a>, Refusal> {
        admission.reserve(0)?;
        admission.units(1)?;
        if !self.matches_program(program) {
            return admission.refuse(Refusal::Subject);
        }
        let key = function as *const _ as usize;
        // Pointer ranks never change acceptance work: pay a fixed upper path.
        let probes = if self.ordinary_positions.is_empty() {
            0
        } else {
            usize::BITS as usize - self.ordinary_positions.len().leading_zeros() as usize
        };
        admission.units(probes)?;
        let (mut begin, mut end) = (0, self.ordinary_positions.len());
        while begin < end {
            let middle = begin + (end - begin) / 2;
            match self.ordinary_positions[middle].0.cmp(&key) {
                std::cmp::Ordering::Less => begin = middle + 1,
                std::cmp::Ordering::Greater => end = middle,
                std::cmp::Ordering::Equal => {
                    return Ok(&self.ordinary[self.ordinary_positions[middle].1])
                }
            }
        }
        admission.refuse(Refusal::Subject)
    }
    pub(crate) fn ordinary_at_with_work(
        &self,
        program: &ResolvedProgram,
        function: &ResolvedFunction,
        at: &ExpressionId,
        work: &mut dyn FnMut(usize) -> bool,
    ) -> Result<Option<(&'a ResolvedBinding, u8)>, Refusal> {
        let mut reserve = crate::bounded_output::reserve_active_required;
        let mut admission = Admission::new(work, &mut reserve);
        let result = (|| {
            if self.ordinary_failed.get() {
                return admission.refuse(Refusal::Work);
            }
            self.ordinary_directory(program, function, &mut admission)?
                .receive(program, function, at, &mut admission)
        })();
        if result.is_err() {
            self.ordinary_failed.set(true);
        }
        result
    }
    pub(crate) fn ordinary_has_with_work(
        &self,
        program: &ResolvedProgram,
        function: &ResolvedFunction,
        family: Family,
        work: &mut dyn FnMut(usize) -> bool,
    ) -> Result<bool, Refusal> {
        let mut reserve = crate::bounded_output::reserve_active_required;
        let mut admission = Admission::new(work, &mut reserve);
        let result = (|| {
            if self.ordinary_failed.get() {
                return admission.refuse(Refusal::Work);
            }
            self.ordinary_directory(program, function, &mut admission)?
                .has_family(family, &mut admission)
        })();
        if result.is_err() {
            self.ordinary_failed.set(true);
        }
        result
    }
    pub(crate) fn ordinary_at(
        &self,
        program: &ResolvedProgram,
        function: &ResolvedFunction,
        at: &ExpressionId,
    ) -> Option<(&'a ResolvedBinding, u8)> {
        self.ordinary_at_with_work(program, function, at, &mut |units| {
            Self::ordinary_units(&self.lookup_work, &self.ordinary_failed, units)
        })
        .ok()
        .flatten()
    }
    pub(crate) fn ordinary_has(
        &self,
        program: &ResolvedProgram,
        function: &ResolvedFunction,
        family: Family,
    ) -> bool {
        self.ordinary_has_with_work(program, function, family, &mut |units| {
            Self::ordinary_units(&self.lookup_work, &self.ordinary_failed, units)
        })
        .unwrap_or(false)
    }
    pub(crate) fn ordinary_complete(&self) -> Result<(), Diagnostic> {
        if self.ordinary_failed.get() {
            Err(resolved::census_error())
        } else {
            Ok(())
        }
    }
}
