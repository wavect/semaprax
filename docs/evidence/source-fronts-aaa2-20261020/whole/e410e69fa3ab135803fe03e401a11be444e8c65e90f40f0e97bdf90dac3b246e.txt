use super::undo;
#[test]
fn complete_original_sources_and_compiled_owning_receivers_remain_exact() {
    {
        let mut current = crate::source_reflows_merge::source36_classifier_input(
            "src/bounded_output/classifier_string.rs",
            include_str!("../../../../bounded_output/classifier_string.rs"),
        );
        undo(&mut current, "//! Physical backing counterpart of the original classifier's logical sink.\n//! Logical decline keeps the original partial/suppressed output; only physical\n//! acquisition publishes a typed failure. No allowance or authority is created.\nuse super::{active, reserve, reserve_sink, Budget};\nuse std::fmt;\nuse std::mem::size_of;\nuse std::rc::Rc;\n\n#[derive(Debug)]\npub(crate) enum GrowthFailure<E> {\n    Payment(E),\n    Arithmetic,\n    Allocation,\n    Capacity,\n    Formatter,\n}\n\npub(crate) struct ClassifierString {\n    bytes: String,\n    logical: Option<Rc<Budget>>,\n}\nimpl ClassifierString {\n    pub(crate) fn acquire<E>(\n        pay: &mut impl FnMut(usize) -> Result<(), E>,\n    ) -> Result<Self, GrowthFailure<E>> {\n        pay(size_of::<Self>()).map_err(GrowthFailure::Payment)?;\n        Ok(Self {\n            bytes: String::new(),\n            logical: active(),\n        })\n    }\n\n    fn append<E>(\n        &mut self,\n        value: &str,\n        pay: &mut impl FnMut(usize) -> Result<(), E>,\n        logical: bool,\n        strict: bool,\n    ) -> Result<bool, GrowthFailure<E>> {\n        // Keep the original reserve_sink frontier BEFORE any physical request.\n        // A reserved-floor decline is not necessarily sticky physical refusal.\n        if logical {\n            // Strict budgeted_format reserves against its captured budget;\n            // CappedString uses reserve_sink's current-or-captured rule.\n            let accepted = if strict {\n                reserve(self.logical.as_deref(), value.len())\n            } else {\n                reserve_sink(self.logical.as_ref(), value.len())\n            };\n            if !accepted {\n                return Ok(false);\n            }\n        }\n        let required = self\n            .bytes\n            .len()\n            .checked_add(value.len())\n            .ok_or(GrowthFailure::Arithmetic)?;\n        if required > self.bytes.capacity() {\n            let doubled = self\n                .bytes\n                .capacity()\n                .checked_mul(2)\n                .ok_or(GrowthFailure::Arithmetic)?;\n            let target = required.max(doubled).max(8);\n            // The whole new requested buffer is paid while the prior buffer\n            // remains charged. Dropping either carrier never refunds the payer.\n            pay(target).map_err(GrowthFailure::Payment)?;\n            self.bytes\n                .try_reserve_exact(target - self.bytes.len())\n                .map_err(|_| GrowthFailure::Allocation)?;\n            if self.bytes.capacity() != target {\n                return Err(GrowthFailure::Capacity);\n            }\n        }\n        self.bytes.push_str(value);\n        Ok(true)\n    }\n\n    /// Exact CappedString semantics: declined logical appends are suppressed,\n    /// later formatters still run, and the accepted partial bytes are retained.\n    pub(crate) fn capped<E>(\n        &mut self,\n        arguments: fmt::Arguments<'_>,\n        pay: &mut impl FnMut(usize) -> Result<(), E>,\n    ) -> Result<(), GrowthFailure<E>> {\n        self.write(arguments, pay, false, true).map(|_| ())\n    }\n\n    /// Exact budgeted_format semantics: the first logical decline stops its\n    /// formatter and discards the whole string, without inventing sticky state.\n    pub(crate) fn strict<E>(\n        &mut self,\n        arguments: fmt::Arguments<'_>,\n        pay: &mut impl FnMut(usize) -> Result<(), E>,\n    ) -> Result<(), GrowthFailure<E>> {\n        match self.write(arguments, pay, true, true) {\n            Ok(true) => {}\n            Ok(false) => self.bytes = String::new(),\n            Err(GrowthFailure::Formatter) => match &self.logical {\n                Some(budget) => {\n                    budget.overflowed.set(true);\n                    self.bytes = String::new();\n                }\n                None => panic!(\"a formatting trait implementation returned an error\"),\n            },\n            Err(error) => return Err(error),\n        }\n        Ok(())\n    }\n\n    /// Original writes to an ordinary String do not debit logical sink text.\n    /// They still acquire actual physical replacement backing before growth.\n    pub(crate) fn raw<E>(\n        &mut self,\n        arguments: fmt::Arguments<'_>,\n        pay: &mut impl FnMut(usize) -> Result<(), E>,\n    ) -> Result<(), GrowthFailure<E>> {\n        self.write(arguments, pay, false, false).map(|_| ())\n    }\n\n    fn write<E>(\n        &mut self,\n        arguments: fmt::Arguments<'_>,\n        pay: &mut impl FnMut(usize) -> Result<(), E>,\n        strict: bool,\n        logical: bool,\n    ) -> Result<bool, GrowthFailure<E>> {\n        struct Writer<'a, E, P> {\n            output: &'a mut ClassifierString,\n            pay: &'a mut P,\n            strict: bool,\n            logical: bool,\n            logical_refusal: bool,\n            physical: Option<GrowthFailure<E>>,\n        }\n        impl<E, P: FnMut(usize) -> Result<(), E>> fmt::Write for Writer<'_, E, P> {\n            fn write_str(&mut self, value: &str) -> fmt::Result {\n                if self.physical.is_some() || (self.strict && self.logical_refusal) {\n                    return Err(fmt::Error);\n                }\n                match self\n                    .output\n                    .append(value, self.pay, self.logical, self.strict)\n                {\n                    Ok(true) => Ok(()),\n                    Ok(false) => {\n                        self.logical_refusal = true;\n                        if self.strict {\n                            Err(fmt::Error)\n                        } else {\n                            Ok(())\n                        }\n                    }\n                    Err(error) => {\n                        self.physical = Some(error);\n                        Err(fmt::Error)\n                    }\n                }\n            }\n        }\n        let mut writer = Writer {\n            output: self,\n            pay,\n            strict,\n            logical,\n            logical_refusal: false,\n            physical: None,\n        };\n        let outcome = fmt::write(&mut writer, arguments);\n        if let Some(error) = writer.physical {\n            return Err(error);\n        }\n        if writer.strict && writer.logical_refusal {\n            return Ok(false);\n        }\n        outcome.map_err(|_| GrowthFailure::Formatter)?;\n        Ok(true)\n    }\n\n    #[cfg(test)]\n    pub(crate) fn as_str(&self) -> &str {\n        &self.bytes\n    }\n    pub(crate) fn into_string(self) -> String {\n        self.bytes\n    }\n}\n\n/// Exact invocation binding only; this scalar/Rc carrier grants no allowance,\n/// cached admission or source authority. Its caller pays its containing control.\npub(crate) struct ClassifierLedger(Option<Rc<Budget>>);\nimpl ClassifierLedger {\n    /// A test-account caller authenticates its own invocation separately. This\n    /// detached marker contains no ACTIVE handle and grants no ordinary payer.\n    #[cfg(test)]\n    pub(crate) fn oracle_marker() -> Self {\n        Self(None)\n    }\n\n    pub(crate) fn capture() -> Self {\n        Self(active())\n    }\n    /// Immutable diagnostic data from this held invocation. It grants neither\n    /// permission to spend nor authority to publish under another ACTIVE owner.\n    pub(crate) fn first_refusal(&self) -> Option<super::ReservationRefusal> {\n        self.0\n            .as_ref()\n            .and_then(|budget| budget.first_refusal.get())\n    }\n    pub(crate) fn current(&self) -> bool {\n        let current = active();\n        let same = match (&self.0, current) {\n            (None, None) => true,\n            (Some(held), Some(current)) => Rc::ptr_eq(held, &current),\n            _ => false,\n        };\n        if !same {\n            if let Some(held) = &self.0 {\n                super::record_refusal(held, 0);\n                held.overflowed.set(true);\n            }\n        }\n        same\n    }\n}\n\n#[cfg(test)]\nmod tests;\n\n#[cfg(test)]\nmod explicit;\n\nmod inventory_work;\n\n#[cfg(test)]\nmod oracle_work;\n", "//! Physical backing counterpart of the original classifier's logical sink.\n//! Logical decline keeps the original partial/suppressed output; only physical\n//! acquisition publishes a typed failure. No allowance or authority is created.\nuse super::{active, reserve, reserve_sink, Budget};\nuse std::fmt;\nuse std::mem::size_of;\nuse std::rc::Rc;\n\n#[derive(Debug)]\npub(crate) enum GrowthFailure<E> {\n    Payment(E),\n    Arithmetic,\n    Allocation,\n    Capacity,\n    Formatter,\n}\n\npub(crate) struct ClassifierString {\n    bytes: String,\n    logical: Option<Rc<Budget>>,\n}\nimpl ClassifierString {\n    pub(crate) fn acquire<E>(\n        pay: &mut impl FnMut(usize) -> Result<(), E>,\n    ) -> Result<Self, GrowthFailure<E>> {\n        pay(size_of::<Self>()).map_err(GrowthFailure::Payment)?;\n        Ok(Self {\n            bytes: String::new(),\n            logical: active(),\n        })\n    }\n\n    fn append<E>(\n        &mut self,\n        value: &str,\n        pay: &mut impl FnMut(usize) -> Result<(), E>,\n        logical: bool,\n        strict: bool,\n    ) -> Result<bool, GrowthFailure<E>> {\n        // Keep the original reserve_sink frontier BEFORE any physical request.\n        // A reserved-floor decline is not necessarily sticky physical refusal.\n        if logical {\n            // Strict budgeted_format reserves against its captured budget;\n            // CappedString uses reserve_sink's current-or-captured rule.\n            let accepted = if strict {\n                reserve(self.logical.as_deref(), value.len())\n            } else {\n                reserve_sink(self.logical.as_ref(), value.len())\n            };\n            if !accepted {\n                return Ok(false);\n            }\n        }\n        let required = self\n            .bytes\n            .len()\n            .checked_add(value.len())\n            .ok_or(GrowthFailure::Arithmetic)?;\n        if required > self.bytes.capacity() {\n            let doubled = self\n                .bytes\n                .capacity()\n                .checked_mul(2)\n                .ok_or(GrowthFailure::Arithmetic)?;\n            let target = required.max(doubled).max(8);\n            // The whole new requested buffer is paid while the prior buffer\n            // remains charged. Dropping either carrier never refunds the payer.\n            pay(target).map_err(GrowthFailure::Payment)?;\n            self.bytes\n                .try_reserve_exact(target - self.bytes.len())\n                .map_err(|_| GrowthFailure::Allocation)?;\n            if self.bytes.capacity() != target {\n                return Err(GrowthFailure::Capacity);\n            }\n        }\n        self.bytes.push_str(value);\n        Ok(true)\n    }\n\n    /// Exact CappedString semantics: declined logical appends are suppressed,\n    /// later formatters still run, and the accepted partial bytes are retained.\n    pub(crate) fn capped<E>(\n        &mut self,\n        arguments: fmt::Arguments<'_>,\n        pay: &mut impl FnMut(usize) -> Result<(), E>,\n    ) -> Result<(), GrowthFailure<E>> {\n        self.write(arguments, pay, false, true).map(|_| ())\n    }\n\n    /// Exact budgeted_format semantics: the first logical decline stops its\n    /// formatter and discards the whole string, without inventing sticky state.\n    pub(crate) fn strict<E>(\n        &mut self,\n        arguments: fmt::Arguments<'_>,\n        pay: &mut impl FnMut(usize) -> Result<(), E>,\n    ) -> Result<(), GrowthFailure<E>> {\n        match self.write(arguments, pay, true, true) {\n            Ok(true) => {}\n            Ok(false) => self.bytes = String::new(),\n            Err(GrowthFailure::Formatter) => match &self.logical {\n                Some(budget) => {\n                    budget.overflowed.set(true);\n                    self.bytes = String::new();\n                }\n                None => panic!(\"a formatting trait implementation returned an error\"),\n            },\n            Err(error) => return Err(error),\n        }\n        Ok(())\n    }\n\n    /// Original writes to an ordinary String do not debit logical sink text.\n    /// They still acquire actual physical replacement backing before growth.\n    pub(crate) fn raw<E>(\n        &mut self,\n        arguments: fmt::Arguments<'_>,\n        pay: &mut impl FnMut(usize) -> Result<(), E>,\n    ) -> Result<(), GrowthFailure<E>> {\n        self.write(arguments, pay, false, false).map(|_| ())\n    }\n\n    fn write<E>(\n        &mut self,\n        arguments: fmt::Arguments<'_>,\n        pay: &mut impl FnMut(usize) -> Result<(), E>,\n        strict: bool,\n        logical: bool,\n    ) -> Result<bool, GrowthFailure<E>> {\n        struct Writer<'a, E, P> {\n            output: &'a mut ClassifierString,\n            pay: &'a mut P,\n            strict: bool,\n            logical: bool,\n            logical_refusal: bool,\n            physical: Option<GrowthFailure<E>>,\n        }\n        impl<E, P: FnMut(usize) -> Result<(), E>> fmt::Write for Writer<'_, E, P> {\n            fn write_str(&mut self, value: &str) -> fmt::Result {\n                if self.physical.is_some() || (self.strict && self.logical_refusal) {\n                    return Err(fmt::Error);\n                }\n                match self\n                    .output\n                    .append(value, self.pay, self.logical, self.strict)\n                {\n                    Ok(true) => Ok(()),\n                    Ok(false) => {\n                        self.logical_refusal = true;\n                        if self.strict {\n                            Err(fmt::Error)\n                        } else {\n                            Ok(())\n                        }\n                    }\n                    Err(error) => {\n                        self.physical = Some(error);\n                        Err(fmt::Error)\n                    }\n                }\n            }\n        }\n        let mut writer = Writer {\n            output: self,\n            pay,\n            strict,\n            logical,\n            logical_refusal: false,\n            physical: None,\n        };\n        let outcome = fmt::write(&mut writer, arguments);\n        if let Some(error) = writer.physical {\n            return Err(error);\n        }\n        if writer.strict && writer.logical_refusal {\n            return Ok(false);\n        }\n        outcome.map_err(|_| GrowthFailure::Formatter)?;\n        Ok(true)\n    }\n\n    #[cfg(test)]\n    pub(crate) fn as_str(&self) -> &str {\n        &self.bytes\n    }\n    pub(crate) fn into_string(self) -> String {\n        self.bytes\n    }\n}\n\n/// Exact invocation binding only; this scalar/Rc carrier grants no allowance,\n/// cached admission or source authority. Its caller pays its containing control.\npub(crate) struct ClassifierLedger(Option<Rc<Budget>>);\nimpl ClassifierLedger {\n    /// A test-account caller authenticates its own invocation separately. This\n    /// detached marker contains no ACTIVE handle and grants no ordinary payer.\n    #[cfg(test)]\n    pub(crate) fn oracle_marker() -> Self {\n        Self(None)\n    }\n\n    pub(crate) fn capture() -> Self {\n        Self(active())\n    }\n    pub(crate) fn current(&self) -> bool {\n        let current = active();\n        let same = match (&self.0, current) {\n            (None, None) => true,\n            (Some(held), Some(current)) => Rc::ptr_eq(held, &current),\n            _ => false,\n        };\n        if !same {\n            if let Some(held) = &self.0 {\n                super::record_refusal(held, 0);\n                held.overflowed.set(true);\n            }\n        }\n        same\n    }\n}\n\n#[cfg(test)]\nmod tests;\n\n#[cfg(test)]\nmod explicit;\n\nmod inventory_work;\n\n#[cfg(test)]\nmod oracle_work;\n");
        undo(
            &mut current,
            r#"mod explicit;

mod inventory_work;

#[cfg(test)]
mod oracle_work;
"#,
            r#"mod explicit;

mod inventory_work;
"#,
        );
        undo(
            &mut current,
            r#"/// cached admission or source authority. Its caller pays its containing control.
pub(crate) struct ClassifierLedger(Option<Rc<Budget>>);
impl ClassifierLedger {
    /// A test-account caller authenticates its own invocation separately. This
    /// detached marker contains no ACTIVE handle and grants no ordinary payer.
    #[cfg(test)]
    pub(crate) fn oracle_marker() -> Self {
        Self(None)
    }

    pub(crate) fn capture() -> Self {
        Self(active())
    }
"#,
            r#"/// cached admission or source authority. Its caller pays its containing control.
pub(crate) struct ClassifierLedger(Option<Rc<Budget>>);
impl ClassifierLedger {
    pub(crate) fn capture() -> Self {
        Self(active())
    }
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__bounded_output__classifier_string.rs.before"
            ),
            "src/bounded_output/classifier_string.rs"
        );
    }
    {
        let mut current = crate::source_reflows_merge::current344::project(
            "src/cleanup_plan.rs",
            include_str!("../../../../cleanup_plan.rs"),
        );
        undo(
            &mut current,
            "\n#[cfg(test)]\npub(crate) use replay::EpsilonAppCensus;\n",
            "",
        );
        undo(
            &mut current,
            r#"
// Shared pinned compiler container layout, not cleanup authority.
pub(crate) use replay::replay_btree_node_upper as compiler_btree_node_upper;

#[cfg(test)]
pub(crate) use replay::with_financed_test_oracles;
"#,
            r#"
// Shared pinned compiler container layout, not cleanup authority.
pub(crate) use replay::replay_btree_node_upper as compiler_btree_node_upper;
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan.rs.before"
            ),
            "src/cleanup_plan.rs"
        );
    }
    {
        let mut current = super::current_view::project(
            "src/cleanup_plan/replay.rs",
            include_str!("../../../replay.rs"),
        );
        undo(&mut current, "    let status_cases = expression_facts::receiving::StatusCases::validate(program, function, budget)?;", "    validate_required_status_sources(program, function)?;");
        undo(&mut current, "    let statuses = expression_facts::receiving::validate_status_sources_certified(\n        function, &expression_facts, budget, &status_cases,", "    let statuses = expression_facts::receiving::validate_status_sources_retained(\n        function, &expression_facts, budget,");
        undo(
            &mut current,
            "\n#[cfg(test)]\npub(crate) use factored::EpsilonAppCensus;\n",
            "",
        );
        undo(&mut current, "    let mut continuations = continuation_storage::Storage::for_program(full, &mut budget)?;\n", "");
        undo(&mut current, "    let mut budget = ReplayBudget::new();\n    let helpers = ordinary_renewals::index(full, &mut budget)?;", "    let mut budget = ReplayBudget::new();\n    let mut continuations = continuation_storage::Storage::new(full);\n    let helpers = ordinary_renewals::index(full, &mut budget)?;");
        undo(&mut current, "    expression_facts::receiving::validate_exits_retained(\n        program,\n        function,\n        &storage,\n        &leaves,\n        &BTreeSet::new(),\n        budget,\n        &statuses,\n    )?;\n", "    expression_facts::receiving::validate_exits(\n        program,\n        function,\n        &storage,\n        &leaves,\n        &BTreeSet::new(),\n        budget,\n    )?;\n");
        undo(
            &mut current,
            r#"    let statuses = expression_facts::receiving::validate_status_sources_retained(
        function, &expression_facts, budget,
    )?;"#,
            r#"    expression_facts::receiving::validate_status_sources(function, &expression_facts, budget)?;"#,
        );
        undo(
            &mut current,
            "    expression_facts::receiving::validate_blocks_and_edges_retained(\n",
            "    expression_facts::receiving::validate_blocks_and_edges(\n",
        );
        undo(
            &mut current,
            "        budget,\n        &statuses,\n    )?;",
            "        budget,\n    )?;",
        );
        undo(
            &mut current,
            "pub(crate) use live_oracle::with_financed_test_oracles;",
            "pub(super) use live_oracle::with_financed_test_oracles;",
        );
        undo(
            &mut current,
            r#"
#[cfg(test)]
pub(super) mod phase_measurement;

#[cfg(test)]
mod live_oracle;
#[cfg(test)]
pub(super) use live_oracle::with_financed_test_oracles;
"#,
            r#"
#[cfg(test)]
pub(super) mod phase_measurement;
"#,
        );
        undo(
            &mut current,
            r####"    expression_facts::receiving::validate_status_sources(function, &expression_facts, budget)?;
    expression_facts::receiving::validate_regions(function, &storage, budget)?;
    expression_facts::receiving::validate_entry(function, &storage, &leaves, budget)?;
    expression_facts::receiving::validate_blocks_and_edges(
"####,
            r####"    expression_facts::receiving::validate_status_sources(function, &expression_facts, budget)?;
    validate_regions(function, &storage)?;
    validate_entry(function, &storage, &leaves)?;
    expression_facts::receiving::validate_blocks_and_edges(
"####,
        );
        undo(
            &mut current,
            r####"    )?;
    expression_facts::receiving::validate_exits(
        program,
        function,
        &storage,
        &leaves,
        &BTreeSet::new(),
        budget,
    )?;
    structural_maps::validate_reference_coverage(function, budget)?;
"####,
            r####"    )?;
    validate_exits(program, function, &storage, &leaves, &BTreeSet::new())?;
    structural_maps::validate_reference_coverage(function, budget)?;
"####,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay.rs.before"
            ),
            "src/cleanup_plan/replay.rs"
        );
    }
    {
        let mut current = super::current_view::project(
            "src/cleanup_plan/replay/allocation.rs",
            include_str!("../../allocation.rs"),
        );
        undo(
            &mut current,
            r#"mod oracle;
#[cfg(test)]
pub(super) use oracle::{
    InvocationRefusal, OracleLease, OracleReceipt, OracleRefusal, OracleScope, ReplayFailure,
    authority, authority_for_function, enter, opt_in, opted, raw_error_funded, raw_error_metered,
    replay_error_funded,
};
pub(super) fn set_key<K: Ord + ClonePayload>(
    function: &ResolvedFunction,
"#,
            r#"mod oracle;
#[cfg(test)]
pub(super) use oracle::{
    authority, authority_for_function, enter, opt_in, opted, raw_error_funded, replay_error_funded,
    InvocationRefusal, OracleLease, OracleReceipt, OracleRefusal, OracleScope, ReplayFailure,
};
pub(super) fn set_key<K: Ord + ClonePayload>(
    function: &ResolvedFunction,
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__allocation.rs.before"
            ),
            "src/cleanup_plan/replay/allocation.rs"
        );
    }
    {
        let mut current = super::current_view::project(
            "src/cleanup_plan/replay/allocation/oracle.rs",
            include_str!("../../allocation/oracle.rs"),
        );
        undo(
            &mut current,
            r#"mod tests;

pub(in crate::cleanup_plan::replay) use diagnostic::{
    ReplayFailure, raw_error_funded, raw_error_metered, replay_error_funded,
};
pub(in crate::cleanup_plan::replay) use invocation::{
    authority, authority_for_function, enter, opt_in, opted, InvocationRefusal,
"#,
            r#"mod tests;

pub(in crate::cleanup_plan::replay) use diagnostic::{
    raw_error_funded, replay_error_funded, ReplayFailure,
};
pub(in crate::cleanup_plan::replay) use invocation::{
    authority, authority_for_function, enter, opt_in, opted, InvocationRefusal,
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__allocation__oracle.rs.before"
            ),
            "src/cleanup_plan/replay/allocation/oracle.rs"
        );
    }
    {
        let mut current = include_str!("../../allocation/oracle/diagnostic.rs").to_owned();
        undo(
            &mut current,
            r#"    }
    Ok(Diagnostic::io("SPX-H006", text))
}

mod metered;
pub(in crate::cleanup_plan::replay) use metered::raw as raw_error_metered;
"#,
            r#"    }
    Ok(Diagnostic::io("SPX-H006", text))
}
"#,
        );
        undo(
            &mut current,
            r#"    lease: OracleLease,
    message: fmt::Arguments<'_>,
) -> Result<Diagnostic, ReplayFailure> {
    if !matches!(super::super::opted(), Ok(None)) {
        return Err(physical(
            lease,
            "opted oracle raw diagnostic requires its metered receiver",
        ));
    }
    let mut count = Count(0);
    if write!(count, "{message}").is_err() {
        return Err(physical(lease, "raw diagnostic length overflowed"));
"#,
            r#"    lease: OracleLease,
    message: fmt::Arguments<'_>,
) -> Result<Diagnostic, ReplayFailure> {
    let mut count = Count(0);
    if write!(count, "{message}").is_err() {
        return Err(physical(lease, "raw diagnostic length overflowed"));
"#,
        );
        undo(
            &mut current,
            r#"    function: &ResolvedFunction,
    message: fmt::Arguments<'_>,
) -> Result<Diagnostic, ReplayFailure> {
    match super::super::opted() {
        Ok(None) => {}
        Ok(Some(current)) if current == lease => return metered::replay(lease, function, message),
        Ok(Some(_)) | Err(_) => {
            return Err(physical(
                lease,
                "opted oracle diagnostic lease is unavailable or differs",
            ));
        }
    }
    let mut count = Count(0);
    if write!(
        count,
"#,
            r#"    function: &ResolvedFunction,
    message: fmt::Arguments<'_>,
) -> Result<Diagnostic, ReplayFailure> {
    let mut count = Count(0);
    if write!(
        count,
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__allocation__oracle__diagnostic.rs.before"
            ),
            "src/cleanup_plan/replay/allocation/oracle/diagnostic.rs"
        );
    }
    {
        let mut current = super::current_view::project(
            "src/cleanup_plan/replay/factored.rs",
            include_str!("../../factored.rs"),
        );
        undo(
            &mut current,
            "pub(crate) use canonical_shared::app_census::Scope as EpsilonAppCensus;",
            "pub(super) use canonical_shared::app_census::Scope as EpsilonAppCensus;",
        );
        undo(&mut current, "\n#[cfg(test)]\npub(super) use canonical_shared::app_census::Scope as EpsilonAppCensus;\n", "");
        undo(&mut current, "        oracle_hook::cross_check_with_storage(\n            program,\n            function,\n            budget,\n            continuations,\n        )?;", "        oracle_hook::cross_check(program, function);");
        undo(
            &mut current,
            "\n#[cfg(test)]\nmod continuation_receiver;\n",
            "",
        );
        undo(&mut current, "#[cfg(test)]\nenum CanonicalFrame {\n    Enter(Weighted),\n    Exit(Weighted, u64, Vec<(Label, Weighted)>),\n}\n\n", "");
        // Join the complete relocated old receiver before the existing inverse.
        let before = ci_str!("../../factored/canonical_before.rs");
        let original = before
            .strip_prefix(
                r###"//! Complete pre-sharing receiver; its interior is retained verbatim.
use super::*;

"###,
            )
            .unwrap()
            .strip_suffix(
                r###"// The wrapper exposes the unchanged receiver to sibling controls.
pub(super) struct Receiver<'a>(Canonicalizer<'a>);
impl<'a> Receiver<'a> {
    pub(super) fn new(automaton: &'a Automaton, sink: usize) -> Self {
        Self(Canonicalizer::new(automaton, sink))
    }
    pub(super) fn id(
        &mut self,
        function: &ResolvedFunction,
        root: Weighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<usize>, Diagnostic> {
        self.0.id(function, root, budget)
    }
    pub(super) fn states(&self) -> &BTreeMap<Weighted, usize> {
        &self.0.states
    }
    pub(super) fn signatures(&self) -> &BTreeMap<(u64, Vec<(Label, usize)>), usize> {
        &self.0.ids
    }
}
"###,
            )
            .unwrap();
        undo(
            &mut current,
            "#[cfg(test)]\nmod canonical_before;\nmod canonical_shared;\n",
            "",
        );
        undo(
            &mut current,
            "#[cfg(test)]\nuse canonical_before::Receiver as Canonicalizer;\n",
            "",
        );
        undo(
            &mut current,
            "    let mut canonical = canonical_shared::Canonicalizer::new(&automaton, sink);",
            "    let mut canonical = Canonicalizer::new(&automaton, sink);",
        );
        let marker = "/// Test builds compare the factored verdict";
        assert_eq!(current.matches(marker).count(), 1);
        assert_eq!(original.matches("#[rustfmt::skip]\n").count(), 1);
        let original = original.replacen("#[rustfmt::skip]\n", "", 1);
        // The complete preexisting inverse below already restores this same
        // old receiver. Compose that restoration once, then authenticate its
        // entire body against the independent compiled receiver below.
        undo(
            &mut current,
            r#"        assert_eq!(short.allocated, short.freed);
    }
}

#[cfg(test)]
mod oracle_order;
"#,
            r#"        assert_eq!(short.allocated, short.freed);
    }
}
"#,
        );
        undo(
            &mut current,
            r#"        hir_root,
        plan_root,
    } = build_comparison(program, function, budget, helpers, continuations)?;

    #[cfg(test)]
    if budget.oracle_lease().is_some() {
        // The exact reconstructed automaton is unchanged. An Oracle account
        // pays the full general label order; it grants no borrowed-helper lease.
        let mut canonical =
            borrowed_canonical::BorrowedCanonicalizer::new(function, &automaton, sink, budget)?;
        budget.reserve_array::<(usize, u64)>(function, 1, "oracle HIR weighted root")?;
        let Some(hir) = canonical.id(function, vec![(hir_root, 1)], budget)? else {
            return Ok(None);
        };
        budget.reserve_array::<(usize, u64)>(function, 1, "oracle plan weighted root")?;
        let Some(plan) = canonical.id(function, vec![(plan_root, 1)], budget)? else {
            return Ok(None);
        };
        return Ok(Some(hir == plan));
    }

    #[cfg(test)]
    if let Some(lease) = program.borrowed_helper_lease(function) {
"#,
            r#"        hir_root,
        plan_root,
    } = build_comparison(program, function, budget, helpers, continuations)?;

    #[cfg(test)]
    if let Some(lease) = program.borrowed_helper_lease(function) {
"#,
        );
        undo(
            &mut current,
            r#"        &mut ReplayBudget::new(),
    )? {
        #[cfg(test)]
        oracle_hook::cross_check(program, function);
        return Ok(false);
    }
    match compare_with_index_storage(program, function, budget, helpers, continuations) {
"#,
            r#"        &mut ReplayBudget::new(),
    )? {
        #[cfg(test)]
        cross_check(program, function);
        return Ok(false);
    }
    match compare_with_index_storage(program, function, budget, helpers, continuations) {
"#,
        );
        undo(
            &mut current,
            r####"
/// Test builds compare the factored verdict with the enumerating one on every
"####,
            r####"
enum CanonicalFrame {
    Enter(Weighted),
    Exit(Weighted, u64, Vec<(Label, Weighted)>),
}

/// Bottom-up hash-consing of the weighted subset construction. A determinized
/// state's id is its accepted weight together with each label and the id of
/// the state that label leads to, so two states share an id exactly when
/// they accept every observation sequence the same number of times.
struct Canonicalizer<'a> {
    automaton: &'a Automaton,
    sink: usize,
    closures: BTreeMap<usize, Rc<BTreeMap<usize, u64>>>,
    states: BTreeMap<Weighted, usize>,
    ids: BTreeMap<(u64, Vec<(Label, usize)>), usize>,
}

impl<'a> Canonicalizer<'a> {
    fn new(automaton: &'a Automaton, sink: usize) -> Self {
        Self {
            automaton,
            sink,
            closures: BTreeMap::new(),
            states: BTreeMap::new(),
            ids: BTreeMap::new(),
        }
    }

    /// Nodes with moves (or the sink) reachable through epsilon edges, each
    /// with its number of epsilon routes.
    fn closure(
        &mut self,
        function: &ResolvedFunction,
        root: usize,
        budget: &mut ReplayBudget,
    ) -> Result<Option<Rc<BTreeMap<usize, u64>>>, Diagnostic> {
        budget.reserve_array::<(usize, bool)>(function, 1, "factored closure initial cursor")?;
        let mut stack = vec![(root, false)];
        while let Some((node, expanded)) = stack.pop() {
            budget.charge(function, 1, "factored closure cursor")?;
            if self.closures.contains_key(&node) {
                continue;
            }
            let state = &self.automaton.nodes[node];
            if !expanded {
                budget.reserve_vec(function, &mut stack, 1, "factored closure cursors")?;
                stack.push((node, true));
                for target in &state.epsilon {
                    if !self.closures.contains_key(target) {
                        budget.reserve_vec(function, &mut stack, 1, "factored closure cursors")?;
                        stack.push((*target, false));
                    }
                }
                continue;
            }
            let mut closure = BTreeMap::new();
            if !state.moves.is_empty() || node == self.sink {
                budget.reserve_btree_node::<usize, u64>(function, "factored closure node")?;
                closure.insert(node, 1_u64);
            }
            for target in &state.epsilon {
                let Some(inner) = self.closures.get(target) else {
                    return Err(replay_error(function, "cleanup CFG contains a cycle"));
                };
                for (inner_node, routes) in inner.iter() {
                    budget.charge(function, 1, "factored closure route addition")?;
                    if !closure.contains_key(inner_node) {
                        budget
                            .reserve_btree_node::<usize, u64>(function, "factored closure node")?;
                    }
                    let slot = closure.entry(*inner_node).or_insert(0_u64);
                    let Some(sum) = slot.checked_add(*routes) else {
                        return Ok(None);
                    };
                    *slot = sum;
                }
            }
            budget.charge(
                function,
                closure.len().saturating_add(1),
                "factored skeleton closure",
            )?;
            budget.reserve_bytes(
                function,
                physical_clone::rc_backing_bytes::<BTreeMap<usize, u64>>(),
                "factored closure shared header",
            )?;
            budget.reserve_btree_node::<usize, Rc<BTreeMap<usize, u64>>>(
                function,
                "factored closure memo node",
            )?;
            self.closures.insert(node, Rc::new(closure));
        }
        Ok(self.closures.get(&root).cloned())
    }

    /// The accepted weight of `state` and its weighted successor per label.
    #[allow(clippy::type_complexity)]
    fn successors(
        &mut self,
        function: &ResolvedFunction,
        state: &Weighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<(u64, Vec<(Label, Weighted)>)>, Diagnostic> {
        let mut closed = BTreeMap::<usize, u64>::new();
        for (node, weight) in state {
            let Some(closure) = self.closure(function, *node, budget)? else {
                return Ok(None);
            };
            for (inner, routes) in closure.iter() {
                let Some(sum) = routes.checked_mul(*weight).and_then(|product| {
                    closed.get(inner).copied().unwrap_or(0).checked_add(product)
                }) else {
                    return Ok(None);
                };
                budget.charge(function, 1, "factored weighted route addition")?;
                if !closed.contains_key(inner) {
                    budget.reserve_btree_node::<usize, u64>(
                        function,
                        "factored weighted closure node",
                    )?;
                }
                closed.insert(*inner, sum);
            }
        }
        let accepted = closed.get(&self.sink).copied().unwrap_or(0);
        let mut successors = BTreeMap::<Label, BTreeMap<usize, u64>>::new();
        for (node, weight) in &closed {
            let moves = &self.automaton.nodes[*node].moves;
            budget.charge(
                function,
                moves.len().saturating_add(1),
                "factored skeleton subset construction",
            )?;
            for (label, child) in moves {
                if !successors.contains_key(label) {
                    budget.reserve_btree_node::<Label, BTreeMap<usize, u64>>(
                        function,
                        "factored successor label node",
                    )?;
                }
                let targets = successors.entry(label.clone()).or_default();
                if !targets.contains_key(child) {
                    budget.reserve_btree_node::<usize, u64>(
                        function,
                        "factored successor target node",
                    )?;
                }
                let slot = targets.entry(*child).or_insert(0);
                let Some(sum) = slot.checked_add(*weight) else {
                    return Ok(None);
                };
                *slot = sum;
            }
        }
        budget.reserve_array::<(Label, Weighted)>(
            function,
            successors.len(),
            "factored successor vector",
        )?;
        let mut result = Vec::with_capacity(successors.len());
        for (label, targets) in successors {
            budget.reserve_array::<(usize, u64)>(
                function,
                targets.len(),
                "factored weighted successor vector",
            )?;
            result.push((label, targets.into_iter().collect()));
        }
        Ok(Some((accepted, result)))
    }

    fn id(
        &mut self,
        function: &ResolvedFunction,
        root: Weighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<usize>, Diagnostic> {
        budget.reserve_array::<CanonicalFrame>(function, 1, "factored canonical initial frame")?;
        let root_copy = storage::clone_weighted(function, budget, &root)?;
        let mut frames = vec![CanonicalFrame::Enter(root_copy)];
        let mut open = BTreeSet::new();
        while let Some(frame) = frames.pop() {
            budget.charge(function, 1, "factored canonical frame")?;
            match frame {
                CanonicalFrame::Enter(state) => {
                    if self.states.contains_key(&state) {
                        continue;
                    }
                    if open.contains(&state) {
                        return Err(replay_error(function, "cleanup CFG contains a cycle"));
                    }
                    budget.reserve_btree_node::<Weighted, ()>(
                        function,
                        "factored open-state identity node",
                    )?;
                    open.insert(storage::clone_weighted(function, budget, &state)?);
                    let Some((accepted, successors)) = self.successors(function, &state, budget)?
                    else {
                        return Ok(None);
                    };
                    let child_count = successors.len();
                    budget.reserve_vec(
                        function,
                        &mut frames,
                        1,
                        "factored canonical exit frame",
                    )?;
                    let exit_index = frames.len();
                    frames.push(CanonicalFrame::Exit(state, accepted, successors));
                    for index in 0..child_count {
                        let CanonicalFrame::Exit(_, _, successors) = &frames[exit_index] else {
                            unreachable!("canonical exit retained below child frames");
                        };
                        let child =
                            storage::clone_weighted(function, budget, &successors[index].1)?;
                        budget.reserve_vec(
                            function,
                            &mut frames,
                            1,
                            "factored canonical child frames",
                        )?;
                        frames.push(CanonicalFrame::Enter(child));
                    }
                }
                CanonicalFrame::Exit(state, accepted, successors) => {
                    open.remove(&state);
                    budget.charge(
                        function,
                        successors.len(),
                        "factored canonical signature construction",
                    )?;
                    budget.reserve_array::<(Label, usize)>(
                        function,
                        successors.len(),
                        "factored canonical signature vector",
                    )?;
                    let mut key = Vec::with_capacity(successors.len());
                    for (label, child) in successors {
                        let Some(id) = self.states.get(&child) else {
                            return Err(replay_error(function, "cleanup CFG contains a cycle"));
                        };
                        key.push((label, *id));
                    }
                    let key = (accepted, key);
                    let id = match self.ids.get(&key) {
                        Some(id) => *id,
                        None => {
                            budget.reserve_btree_node::<(u64, Vec<(Label, usize)>), usize>(
                                function,
                                "factored canonical signature node",
                            )?;
                            let id = self.ids.len();
                            self.ids.insert(key, id);
                            id
                        }
                    };
                    if !self.states.contains_key(&state) {
                        budget.reserve_btree_node::<Weighted, usize>(
                            function,
                            "factored canonical state memo node",
                        )?;
                    }
                    self.states.insert(state, id);
                }
            }
        }
        Ok(self.states.get(&root).copied())
    }
}

/// Test builds compare the factored verdict with the enumerating one on every
"####,
        );
        let joined = format!("{original}{marker}");
        assert_eq!(
            current.matches(&joined).count(),
            1,
            "complete compiled old canonical receiver remains byte-exact"
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__factored.rs.before"
            ),
            "src/cleanup_plan/replay/factored.rs"
        );
    }
    {
        let mut current = crate::source_reflows_merge::current344::project(
            "src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs",
            include_str!("../../factored/borrowed_canonical/dictionary/keys.rs"),
        );
        undo(
            &mut current,
            r#"        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Ordering, Diagnostic> {
        #[cfg(test)]
        if budget.oracle_lease().is_some() {
            return crate::cleanup_plan::replay::factored::oracle_order::label(
                self, other, function, budget,
            );
        }
        budget.charge(function, 1, "borrowed label actual kind")?;
        let (a, b) = match (self, other) {
            (Label::End(a), Label::End(b)) => return Ok(a.cmp(b)),
"#,
            r#"        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Ordering, Diagnostic> {
        budget.charge(function, 1, "borrowed label actual kind")?;
        let (a, b) = match (self, other) {
            (Label::End(a), Label::End(b)) => return Ok(a.cmp(b)),
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__factored__borrowed_canonical__dictionary__keys.rs.before"
            ),
            "src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs"
        );
    }
    complete_original_sources_tail_a();
}

include!("a_tail.rs");
