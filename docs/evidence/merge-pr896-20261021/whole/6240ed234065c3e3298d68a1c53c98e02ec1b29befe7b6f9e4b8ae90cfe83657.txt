use super::*;
use crate::cleanup_plan::renewal_history::RenewalHistory;
use crate::cleanup_plan::replay::inventory_coverage::paid::receive_observed as receive;
use std::mem::size_of;
type NewCaller<'a> = (
    &'a ResolvedFunction,
    Vec<LivenessFlagId>,
    Option<LivenessFlagId>,
    &'a mut ReplayBudget,
    Result<RenewalHistory, Error>,
);
type NewFormal<'a> = (
    &'a ResolvedFunction,
    Vec<LivenessFlagId>,
    Option<LivenessFlagId>,
    &'a mut ReplayBudget,
    Result<RenewalHistory, Error>,
    Vec<(LivenessFlagId, LivenessFlagId)>,
    Diagnostic,
    Error,
    Diagnostic,
    Error,
);
type Construct = (
    Vec<LivenessFlagId>,
    Vec<(LivenessFlagId, LivenessFlagId)>,
    Option<LivenessFlagId>,
    RenewalHistory,
    Vec<LivenessFlagId>,
    Vec<(LivenessFlagId, LivenessFlagId)>,
    Option<LivenessFlagId>,
    RenewalHistory,
);
type Next<'a> = (
    &'a mut std::slice::Iter<'a, LivenessFlagId>,
    Option<&'a LivenessFlagId>,
    &'a LivenessFlagId,
);
#[test]
fn genuine_nominal_history_zero_one_many_constructor_exact_next_work_and_drop() {
    let fixture = crate::workspace_graph::SourceRecordReplayFixture::new(nominal::SOURCE);
    fixture.with_context(|context| {
        let function = context
            .functions()
            .iter()
            .find(|f| f.id.as_str() == "flat.identity")
            .unwrap();
        let leaves = collect_leaves(function).unwrap();
        let flags = leaves.keys().copied().collect::<Vec<_>>();
        assert!(
            flags.len() >= 2,
            "genuine record contains String and Bytes leaves"
        );
        let initial = ReplayBudget::with_physical(function, LIMIT).unwrap();
        let setup = initial.physical.as_ref().unwrap().charged();
        for n in [0, 1, flags.len()] {
            for protect in [false, true] {
                let protected = protect.then_some(flags[0]);
                let expected = RenewalHistory::new(flags[..n].to_vec(), protected);
                let payload = n * size_of::<LivenessFlagId>()
                    + if protect {
                        n * size_of::<(LivenessFlagId, LivenessFlagId)>()
                    } else {
                        0
                    };
                let request = size_of::<NewCaller<'_>>()
                    + size_of::<NewFormal<'_>>()
                    + size_of::<Construct>()
                    + if protect {
                        size_of::<std::slice::Iter<'_, LivenessFlagId>>()
                            + (n + 1) * size_of::<Next<'_>>()
                            + n * size_of::<(LivenessFlagId, LivenessFlagId)>()
                    } else {
                        0
                    };
                let work = 5 + 10 + 8 + if protect { 1 + 3 * (n + 1) + 2 * n } else { 0 };
                for (bytes, units, success) in [
                    (setup + payload + request, work, true),
                    (setup + payload + request - 1, work, false),
                    (setup + payload + request, work - 1, false),
                    (setup + payload + request, 0, false),
                    (setup + payload, work, false),
                ] {
                    let data = flags[..n].to_vec();
                    let mut budget = ReplayBudget::with_physical(function, bytes).unwrap();
                    budget
                        .reserve_array::<LivenessFlagId>(function, n, "original history backing")
                        .unwrap();
                    if protect {
                        budget
                            .reserve_array::<(LivenessFlagId, LivenessFlagId)>(
                                function,
                                n,
                                "original epoch backing",
                            )
                            .unwrap();
                    }
                    budget.remaining = units;
                    let actual = receive!(
                        budget,
                        function,
                        NewCaller<'_>,
                        5,
                        "selected history test caller receiving",
                        super::super::history::new(function, data, protected, &mut budget)
                    );
                    if success {
                        assert_eq!(actual.unwrap(), expected);
                        assert_eq!(budget.remaining, 0);
                        assert_eq!(
                            budget.physical.as_ref().unwrap().charged(),
                            setup + payload + request
                        );
                    } else {
                        assert!(matches!(actual, Err(Error::Owned(_))));
                        let ledger = budget.physical.as_deref_mut().unwrap();
                        let first = ledger.reserve(0).unwrap_err();
                        assert_eq!(ledger.reserve(1).unwrap_err(), first);
                    }
                }
            }
        }
        let (_, observed) = measure(|| {
            for n in [0, 1, flags.len()] {
                let mut original = RenewalHistory::new(flags[..n].to_vec(), Some(flags[0]));
                let mut current = original.clone();
                let mapping = flags
                    .iter()
                    .copied()
                    .zip(flags.iter().copied().rev())
                    .collect::<Vec<_>>();
                let mut budget = ReplayBudget::with_physical(function, LIMIT).unwrap();
                original.stage(|flag| {
                    mapping
                        .iter()
                        .find(|(source, _)| *source == flag)
                        .map(|(_, target)| *target)
                });
                receive!(
                    budget,
                    function,
                    (
                        &ResolvedFunction,
                        &mut RenewalHistory,
                        &[(LivenessFlagId, LivenessFlagId)],
                        &mut ReplayBudget,
                        Result<(), Error>
                    ),
                    5,
                    "selected history stage test caller receiving",
                    super::super::history::stage(function, &mut current, &mapping, &mut budget)
                )
                .unwrap();
                assert_eq!(
                    current, original,
                    "complete history/epochs/protected after stage n={n}"
                );
                let consumed = BTreeSet::from([flags[0]]);
                original.commit(&consumed);
                receive!(
                    budget,
                    function,
                    (
                        &ResolvedFunction,
                        &mut RenewalHistory,
                        &BTreeSet<LivenessFlagId>,
                        &mut ReplayBudget,
                        Result<(), Error>
                    ),
                    5,
                    "selected history commit test caller receiving",
                    super::super::history::commit(function, &mut current, &consumed, &mut budget)
                )
                .unwrap();
                assert_eq!(
                    current, original,
                    "complete history/epochs/protected after commit n={n}"
                );
            }
        });
        assert!(!observed.invalid && observed.live == 0);
        assert_eq!(observed.allocated, observed.freed);
    });
}

#[test]
fn genuine_history_commit_zero_one_many_comparison_exact_short_zero_sticky_and_drop() {
    type Flag = LivenessFlagId;
    type Pair = (Flag, Flag);
    type Caller<'a> = (
        &'a ResolvedFunction,
        &'a mut RenewalHistory,
        &'a BTreeSet<Flag>,
        &'a mut ReplayBudget,
        Result<(), Error>,
    );
    type Formal<'a> = (
        &'a ResolvedFunction,
        &'a mut RenewalHistory,
        &'a BTreeSet<Flag>,
        &'a mut ReplayBudget,
        Result<(), Error>,
        Diagnostic,
        Error,
        Diagnostic,
        Error,
    );
    type Parts<'a> = (&'a mut Vec<Flag>, &'a mut Vec<Pair>, Option<Flag>);
    type PartsReceiving<'a> = (
        &'a mut RenewalHistory,
        Parts<'a>,
        &'a mut RenewalHistory,
        Parts<'a>,
        &'a mut Vec<Flag>,
        &'a mut Vec<Pair>,
        Option<Flag>,
    );
    type Indices<'a> = (usize, usize, usize, std::slice::Iter<'a, Pair>);
    type Row<'a> = (Flag, &'a Flag, &'a Flag, bool);
    type Get<'a> = (&'a Vec<Flag>, usize, Option<&'a Flag>, &'a Flag);
    type Next<'a> = (
        &'a mut std::slice::Iter<'a, Pair>,
        Option<&'a Pair>,
        &'a Flag,
        &'a Flag,
    );
    type Compare<'a> = (Option<Flag>, &'a Option<Flag>, &'a Option<Flag>, bool);
    type Write<'a> = (&'a mut Vec<Flag>, usize, Option<&'a mut Flag>, &'a mut Flag);
    type EpochGet<'a> = (&'a Vec<Pair>, usize, Option<&'a Pair>, &'a Pair);
    type EpochWrite<'a> = (&'a mut Vec<Pair>, usize, Option<&'a mut Pair>, &'a mut Pair);
    let fixture = crate::workspace_graph::SourceRecordReplayFixture::new(nominal::SOURCE);
    fixture.with_context(|context| {
        let function = context
            .functions()
            .iter()
            .find(|f| f.id.as_str() == "flat.identity")
            .unwrap();
        let flags = collect_leaves(function)
            .unwrap()
            .keys()
            .copied()
            .collect::<Vec<_>>();
        assert!(flags.len() >= 2);
        let (_, observed) = measure(|| {
            for n in [0, 1, flags.len()] {
                // Empty consumed set reaches both comparison and write sites on every row.
                let consumed = BTreeSet::new();
                let mut expected = RenewalHistory::new(flags[..n].to_vec(), Some(flags[0]));
                expected.commit(&consumed);
                let payload = n * (size_of::<Flag>() + size_of::<Pair>());
                let initial = ReplayBudget::with_physical(function, LIMIT).unwrap();
                let setup = initial.physical.as_ref().unwrap().charged();
                drop(initial);
                let request = size_of::<Caller<'_>>()
                    + size_of::<Formal<'_>>()
                    + size_of::<PartsReceiving<'_>>()
                    + if n == 0 {
                        0
                    } else {
                        size_of::<Indices<'_>>()
                            + size_of::<(usize, usize, usize)>()
                            + n * (size_of::<Row<'_>>()
                                + size_of::<Get<'_>>()
                                + size_of::<Next<'_>>()
                                + size_of::<Compare<'_>>()
                                + size_of::<Write<'_>>()
                                + size_of::<(Pair, bool)>()
                                + size_of::<EpochGet<'_>>()
                                + size_of::<Compare<'_>>()
                                + size_of::<EpochWrite<'_>>())
                    };
                // The counted history loop consumes one epoch per leaf, with no terminal next.
                let work = 5
                    + 9
                    + 7
                    + if n == 0 {
                        0
                    } else {
                        4 + 3 + n * (4 + 4 + 4 + 4 + 4 + 2 + 4 + 4 + 4)
                    };
                for (bytes, units, success) in [
                    (setup + payload + request, work, true),
                    (setup + payload + request - 1, work, false),
                    (setup + payload + request, work - 1, false),
                    (setup + payload + request, 0, false),
                    (setup + payload, work, false),
                ] {
                    let mut budget = ReplayBudget::with_physical(function, bytes).unwrap();
                    budget
                        .reserve_array::<Flag>(function, n, "original history backing")
                        .unwrap();
                    budget
                        .reserve_array::<Pair>(function, n, "original epoch backing")
                        .unwrap();
                    let mut current = RenewalHistory::new(flags[..n].to_vec(), Some(flags[0]));
                    budget.remaining = units;
                    let actual = receive!(
                        budget,
                        function,
                        Caller<'_>,
                        5,
                        "selected commit exact test caller receiving",
                        super::super::history::commit(
                            function,
                            &mut current,
                            &consumed,
                            &mut budget
                        )
                    );
                    if success {
                        actual.unwrap();
                        assert_eq!(
                            current, expected,
                            "full history/epoch/protected state n={n}"
                        );
                        assert_eq!(budget.remaining, 0);
                        assert_eq!(
                            budget.physical.as_ref().unwrap().charged(),
                            setup + payload + request
                        );
                    } else {
                        assert!(matches!(actual, Err(Error::Owned(_))));
                        let ledger = budget.physical.as_deref_mut().unwrap();
                        let first = ledger.reserve(0).unwrap_err();
                        assert_eq!(ledger.reserve(1).unwrap_err(), first);
                    }
                }
            }
        });
        assert!(!observed.invalid && observed.live == 0);
        assert_eq!(observed.allocated, observed.freed);
    });
}
