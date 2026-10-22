//! Selected renewal provenance loops; original RenewalHistory APIs stay intact.
//! New helper frames also admit both sides of the one terminal budget
//! Diagnostic -> Error conversion, preserving original prepaid text ownership.
use super::*;
use crate::cleanup_plan::renewal_history::RenewalHistory;
type Parts<'a> = (
    &'a mut Vec<LivenessFlagId>,
    &'a mut Vec<(LivenessFlagId, LivenessFlagId)>,
    Option<LivenessFlagId>,
);
macro_rules! parts {
    ($budget:ident,$function:ident,$history:ident) => {
        receive!(
            $budget,
            $function,
            (
                &mut RenewalHistory,
                Parts<'_>,
                &mut RenewalHistory,
                Parts<'_>,
                &mut Vec<LivenessFlagId>,
                &mut Vec<(LivenessFlagId, LivenessFlagId)>,
                Option<LivenessFlagId>
            ),
            7,
            "selected renewal parts caller and callee receiving",
            $history.selected_parts()
        )
    };
}
pub(super) fn new(
    function: &ResolvedFunction,
    history: Vec<LivenessFlagId>,
    protected: Option<LivenessFlagId>,
    budget: &mut ReplayBudget,
) -> Result<RenewalHistory, Error> {
    receive!(
        budget,
        function,
        (
            &ResolvedFunction,
            Vec<LivenessFlagId>,
            Option<LivenessFlagId>,
            &mut ReplayBudget,
            Result<RenewalHistory, Error>,
            Vec<(LivenessFlagId, LivenessFlagId)>,
            Diagnostic,
            Error,
            Diagnostic,
            Error
        ),
        10,
        "selected renewal constructor formal and epoch vector receiving",
        ()
    );
    // Pattern dispatch inspects the persistent Option without a new method call.
    #[allow(clippy::redundant_pattern_matching)]
    let mut epochs = if let Some(_) = protected {
        Vec::with_capacity(history.len())
    } else {
        Vec::new()
    };
    #[allow(clippy::redundant_pattern_matching)]
    if let Some(_) = protected {
        receive!(
            budget,
            function,
            std::slice::Iter<'_, LivenessFlagId>,
            1,
            "selected renewal constructor iterator receiving",
            ()
        );
        let mut flags = history.iter();
        while let Some(flag) = dynamic_next!(
            budget,
            function,
            flags,
            std::slice::Iter<'_, LivenessFlagId>,
            &LivenessFlagId,
            (&LivenessFlagId),
            3
        ) {
            receive!(
                budget,
                function,
                (LivenessFlagId, LivenessFlagId),
                2,
                "selected renewal epoch pair receiving",
                ()
            );
            epochs.push((*flag, *flag));
        }
    }
    Ok(receive!(
        budget,
        function,
        (
            Vec<LivenessFlagId>,
            Vec<(LivenessFlagId, LivenessFlagId)>,
            Option<LivenessFlagId>,
            RenewalHistory,
            Vec<LivenessFlagId>,
            Vec<(LivenessFlagId, LivenessFlagId)>,
            Option<LivenessFlagId>,
            RenewalHistory
        ),
        8,
        "selected renewal construction caller and callee receiving",
        RenewalHistory::from_selected_parts(history, epochs, protected)
    ))
}
pub(super) fn stage(
    function: &ResolvedFunction,
    history: &mut RenewalHistory,
    mapping: &[(LivenessFlagId, LivenessFlagId)],
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    receive!(
        budget,
        function,
        (
            &ResolvedFunction,
            &mut RenewalHistory,
            &[(LivenessFlagId, LivenessFlagId)],
            &mut ReplayBudget,
            Result<(), Error>,
            Diagnostic,
            Error,
            Diagnostic,
            Error
        ),
        9,
        "selected renewal staging formal receiving",
        ()
    );
    let (_, epochs, _) = parts!(budget, function, history);
    receive!(
        budget,
        function,
        std::slice::IterMut<'_, (LivenessFlagId, LivenessFlagId)>,
        1,
        "selected renewal staging iterator receiving",
        ()
    );
    let mut entries = epochs.iter_mut();
    while let Some((_, current)) = dynamic_next!(
        budget,
        function,
        entries,
        std::slice::IterMut<'_, (LivenessFlagId, LivenessFlagId)>,
        &mut (LivenessFlagId, LivenessFlagId),
        (&mut LivenessFlagId),
        3
    ) {
        if let Some(pair) = find!(
            budget,
            function,
            mapping.iter(),
            std::slice::Iter<'_, (LivenessFlagId, LivenessFlagId)>,
            &(LivenessFlagId, LivenessFlagId),
            pair,
            (&(LivenessFlagId, LivenessFlagId)),
            3,
            pair.0 == *current
        ) {
            *current = pair.1;
        }
    }
    Ok(())
}
pub(super) fn commit(
    function: &ResolvedFunction,
    history: &mut RenewalHistory,
    consumed: &BTreeSet<LivenessFlagId>,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    receive!(
        budget,
        function,
        (
            &ResolvedFunction,
            &mut RenewalHistory,
            &BTreeSet<LivenessFlagId>,
            &mut ReplayBudget,
            Result<(), Error>,
            Diagnostic,
            Error,
            Diagnostic,
            Error
        ),
        9,
        "selected renewal commit formal receiving",
        ()
    );
    let (history, epochs, protected) = parts!(budget, function, history);
    if epochs.is_empty() {
        return Ok(());
    }
    receive!(
        budget,
        function,
        (
            usize,
            usize,
            usize,
            std::slice::Iter<'_, (LivenessFlagId, LivenessFlagId)>
        ),
        4,
        "selected renewal commit history indices and epoch iterator receiving",
        ()
    );
    let mut read = 0usize;
    let mut write = 0usize;
    let length = history.len();
    let mut entries = epochs.iter();
    while read < length {
        receive!(
            budget,
            function,
            (LivenessFlagId, &LivenessFlagId, &LivenessFlagId, bool),
            4,
            "selected renewal commit original and predicate receiving",
            ()
        );
        let original = *dynamic_get!(
            budget,
            function,
            history,
            &Vec<LivenessFlagId>,
            read,
            usize,
            Option<&LivenessFlagId>,
            (&LivenessFlagId),
            4
        )
        .expect("history index in bounds");
        let (expected, current) = dynamic_next!(
            budget,
            function,
            entries,
            std::slice::Iter<'_, (LivenessFlagId, LivenessFlagId)>,
            &(LivenessFlagId, LivenessFlagId),
            (&LivenessFlagId, &LivenessFlagId),
            4
        )
        .expect("reserved provenance accompanies each original leaf");
        debug_assert_eq!(&original, expected);
        if receive!(
            budget,
            function,
            (
                Option<LivenessFlagId>,
                &Option<LivenessFlagId>,
                &Option<LivenessFlagId>,
                bool
            ),
            4,
            "selected renewal fresh protected comparison receiving",
            Some(original) == protected
        ) || !consumed.contains(current)
        {
            *receive!(
                budget,
                function,
                (
                    &mut Vec<LivenessFlagId>,
                    usize,
                    Option<&mut LivenessFlagId>,
                    &mut LivenessFlagId
                ),
                4,
                "selected renewal commit history write receiving",
                history.get_mut(write)
            )
            .expect("compacted history index in bounds") = original;
            write += 1;
        }
        read += 1;
    }
    history.truncate(write);
    // Original commit consumes exactly one epoch per original history item;
    // it does not probe a terminal epoch None at this non-iterator loop boundary.
    receive!(
        budget,
        function,
        (usize, usize, usize),
        3,
        "selected renewal commit epoch indices receiving",
        ()
    );
    let mut read = 0usize;
    let mut write = 0usize;
    let length = epochs.len();
    while read < length {
        receive!(
            budget,
            function,
            ((LivenessFlagId, LivenessFlagId), bool),
            2,
            "selected renewal commit epoch predicate receiving",
            ()
        );
        let pair = *dynamic_get!(
            budget,
            function,
            epochs,
            &Vec<(LivenessFlagId, LivenessFlagId)>,
            read,
            usize,
            Option<&(LivenessFlagId, LivenessFlagId)>,
            (&(LivenessFlagId, LivenessFlagId)),
            4
        )
        .expect("epoch index in bounds");
        if receive!(
            budget,
            function,
            (
                Option<LivenessFlagId>,
                &Option<LivenessFlagId>,
                &Option<LivenessFlagId>,
                bool
            ),
            4,
            "selected renewal fresh protected comparison receiving",
            Some(pair.0) == protected
        ) || !consumed.contains(&pair.1)
        {
            *receive!(
                budget,
                function,
                (
                    &mut Vec<(LivenessFlagId, LivenessFlagId)>,
                    usize,
                    Option<&mut (LivenessFlagId, LivenessFlagId)>,
                    &mut (LivenessFlagId, LivenessFlagId)
                ),
                4,
                "selected renewal commit epoch write receiving",
                epochs.get_mut(write)
            )
            .expect("compacted epoch index in bounds") = pair;
            write += 1;
        }
        read += 1;
    }
    epochs.truncate(write);
    Ok(())
}
