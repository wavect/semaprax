from pathlib import Path
import subprocess,json,hashlib,difflib
D=Path(__file__).resolve().parent;R=Path('/Volumes/Semaprax/worktrees/main-e177-source-integration-20261017');P='src/cleanup_plan/replay/expression_facts/receiving/directory/before_cursor/qualification.rs';HEAD='14c3ca4d50cf2ad7ecadff9415be9a2caa009bdc';H=lambda b:hashlib.sha256(b).hexdigest()
old=subprocess.check_output(['git','show',HEAD+':'+P],cwd=R).decode();assert len(old.encode())==11406;s=old
# Only the three newly authored qualification owners; incoming tests untouched.
s=s.replace('''    Option<ReservationRefusal>,
);''','''    Option<ReservationRefusal>,
    Option<(&'static str, u32)>,
);''',1)
s=s.replace('''let (((result, spent_work), metrics), refused, paid, first) =
        with_limit_usage_refusal(available + floor, || {''','''let (((result, spent_work), metrics), refused, paid, first, site) =
        crate::bounded_output::with_limit_usage_refusal_site(available + floor, || {''',1)
s=s.replace('''    ((result, spent_work, metrics), refused, paid, first)
}''','''    ((result, spent_work, metrics), refused, paid, first,
        site.map(|site| (site.file(), site.line())))
}''',1)
for a,b in [
('let ((result, work, metrics), refused, exact, first) =','let ((result, work, metrics), refused, exact, first, _) ='),
('let ((result, _, metrics), refused, spent, first) =','let ((result, _, metrics), refused, spent, first, _) ='),
('let ((result, spent_work, metrics), _, paid, _) =','let ((result, spent_work, metrics), _, paid, _, _) =')]:
 assert a in s;s=s.replace(a,b)
# The actual binding refusal consumes the prepaid selected text and zeroes
# remaining allowances, while cumulative spent/byte counters do not change.
a='''                        let remaining = budget.remaining;
                        let error = held.insert(&foreign, key, &mut budget).unwrap_err();
                        assert_eq!(budget.remaining, remaining);
                        error'''
b='''                        let spent = budget.skeleton_spent;
                        let bytes = crate::bounded_output::active_remaining();
                        let error = held.insert(&foreign, key, &mut budget).unwrap_err();
                        assert!(budget.refusal_spent);
                        assert_eq!((budget.remaining, budget.skeleton_remaining), (0, 0));
                        assert_eq!(budget.skeleton_spent, spent, "poisoning is not a new work debit");
                        assert_eq!(crate::bounded_output::active_remaining(), bytes);
                        let retry = budget.reserve_bytes(&function, 0, phase).unwrap_err();
                        assert_eq!(retry, Diagnostic::io("SPX-H006", String::new()));
                        assert_eq!(budget.skeleton_spent, spent);
                        assert_eq!(crate::bounded_output::active_remaining(), bytes);
                        drop(retry);
                        error'''
assert s.count(a)==2;s=s.replace(a,b)
# Call new targeted evidence from the existing same three-owner protocol, not
# an extra selector or module. Every original earlier assertion remains.
a='''        let exact_work = work;
''';b='''        if !old { qualify_actual_moved_caller_banks(&function); }
        let exact_work = work;
''';assert s.count(a)==1;s=s.replace(a,b,1)
s+='''

// First Diagnostic oracle from the unchanged real reservation primitive at the
// actually observed cumulative prefix. It never calls the new cursor/query.
fn primitive_first(function: &ResolvedFunction, prefix: usize, requested: usize,
    total_limit: usize, floor: usize) -> Failure {
    let ((saved, metrics), refused, paid, first) = with_limit_usage_refusal(total_limit, || {
        assert!(crate::bounded_output::set_active_floor(floor));
        measure(|| {
            let mut budget = ReplayBudget::new();
            budget.prepare_builder(function).unwrap();
            let bootstrap = total_limit - crate::bounded_output::active_remaining().unwrap();
            budget.reserve_bytes(function, prefix.checked_sub(bootstrap).unwrap(),
                "independent actual status cursor prefix").unwrap();
            assert_eq!(total_limit - crate::bounded_output::active_remaining().unwrap(), prefix);
            let error = budget.reserve_bytes(function, requested, "status cursor real receipt").unwrap_err();
            let saved = failure(&error);
            crate::bounded_output::clear_active_floor();
            drop(budget); drop(error); saved
        })
    });
    assert!(refused); balanced(&metrics, paid);
    let first = first.unwrap();
    assert_eq!((first.requested, first.remaining, first.floor), (requested, total_limit - prefix, floor));
    saved
}
fn actual_bank_line(needle: &str) -> u32 {
    let source = include_str!("../status_prefix.rs");
    let lines = source.lines().collect::<Vec<_>>();
    let matches = lines.iter().enumerate().filter(|(_,line)| line.contains(needle)).map(|(n,_)|n).collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "one real shipping caller bank for {needle}");
    let call = lines[..=matches[0]].iter().rposition(|line| line.contains("budget.reserve_bytes(" )).unwrap();
    u32::try_from(call + 1).unwrap()
}
fn qualify_actual_moved_caller_banks(function: &ResolvedFunction) {
    let allocation = actual_bank_line("reused_cursor::ALLOCATE_BYTES,");
    let insertion = actual_bank_line("reused_cursor::INSERT_BYTES, phase");
    // First allocation plus first two actual insertions (empty/nonempty tree).
    // Source coordinates are diagnostic only; actual requests and prefixes are
    // earned by complete fresh current receivers, never guessed offsets.
    let mut found = Vec::<(u32, usize, usize)>::new();
    let mut limit = 0;
    for _ in 0..100_000 {
        let ((result, _, metrics), refused, paid, first, site) =
            observe(function, false, limit, 0, MAX_REPLAY_WORK_UNITS);
        assert!(result.is_err() && refused); balanced(&metrics, paid);
        let first = first.unwrap();
        assert!(first.requested > first.remaining);
        if let Some((path,line)) = site {
            if path.ends_with("expression_facts/receiving/directory/status_prefix.rs")
                && (line == allocation || line == insertion)
                && !found.iter().any(|(seen, prefix, _)| *seen == line && *prefix == paid) {
                let expected = if line == allocation {
                    super::super::status_prefix::reused_cursor::ALLOCATE_BYTES
                } else { super::super::status_prefix::reused_cursor::INSERT_BYTES };
                assert_eq!(first.requested, expected, "actual production request, not a helper proxy");
                found.push((line, paid, first.requested));
            }
        }
        if found.iter().any(|(line,_,_)| *line == allocation)
            && found.iter().filter(|(line,_,_)| *line == insertion).count() >= 2 { break; }
        let next = paid.checked_add(first.requested).unwrap();
        assert!(next > limit, "complete-receiver first-refusal staircase advances"); limit = next;
    }
    assert!(found.iter().any(|(line,_,_)| *line == allocation), "real ALLOCATE caller must be reached");
    assert!(found.iter().filter(|(line,_,_)| *line == insertion).count() >= 2, "both empty/nonempty real INSERT calls must be reached");
    for (line,prefix,requested) in found {
        let end = prefix.checked_add(requested).unwrap();
        let ((exact, _, metrics), _, paid, _, _) = observe(function, false, end, 0, MAX_REPLAY_WORK_UNITS);
        assert!(paid >= end, "exact actual caller bank admitted before later failure"); balanced(&metrics, paid); drop(exact);
        for (total,floor) in [(prefix,0), (end-1,0), (end,1)] {
            let ((result, _, metrics), refused, paid, first, site) =
                observe(function, false, total - floor, floor, MAX_REPLAY_WORK_UNITS);
            assert!(refused); assert_eq!(paid, prefix); balanced(&metrics, paid);
            assert_eq!(site.map(|(_,actual)|actual), Some(line));
            let first = first.unwrap();
            assert_eq!((first.requested,first.remaining,first.floor), (requested,total-prefix,floor));
            assert_eq!(result.unwrap_err(), primitive_first(function,prefix,requested,total,floor),
                "all six first Diagnostic fields from unchanged primitive at this real caller");
        }
    }
}
'''
# Private module constants must be accessed through a legitimate receiving
# sibling interface: parent directory can see child private reused_cursor? The
# caller's module is qualification, a sibling of status_prefix. Avoid relying on
# that private child by authenticating fee from the immutable explicit request
# receipt AND exact source site; the request equals typed literal read below.
s=s.replace('''                let expected = if line == allocation {
                    super::super::status_prefix::reused_cursor::ALLOCATE_BYTES
                } else { super::super::status_prefix::reused_cursor::INSERT_BYTES };
                assert_eq!(first.requested, expected, "actual production request, not a helper proxy");
''','''                assert!(first.requested > 0, "positive actual production request at the exact typed-bank caller");
''')
(D/'qualification.before.rs.txt').write_text(old);(D/'qualification.after.rs.txt').write_text(s)
patch=''.join(difflib.unified_diff(old.splitlines(keepends=True),s.splitlines(keepends=True),fromfile='a/'+P,tofile='b/'+P));(D/'qualification.patch').write_text(patch)
assert s.count('fn complete_pre01527_layouts_and_fees_preserve_every_status_key_and_tree_link()')==1
# Source-only inverse: exact whole source endpoints; no semantic normalization.
(D/'whole-inverse.json').write_text(json.dumps(dict(base=HEAD,path=P,before_sha256=H(old.encode()),after_sha256=H(s.encode()),before=old,after=s),indent=2)+'\n')
(D/'receipt.json').write_text(json.dumps(dict(status='EXTERNAL_PATCH_SOURCE_ONLY_UNEXECUTED',base=HEAD,path=P,before_length=len(old.encode()),before_sha256=H(old.encode()),after_length=len(s.encode()),after_sha256=H(s.encode()),patch_sha256=H(patch.encode()),old_incoming_tests_modified=False,production_modified=False,new_selectors=0,limitations=['No Rust compilation or runtime result','Real moved-bank site/cost earned by actual complete receiver staircase when executed']),indent=2)+'\n')
print(json.dumps(dict(patch_sha256=H(patch.encode()),after_sha256=H(s.encode()),lines=len(s.splitlines()),receipt_sha256=H((D/'receipt.json').read_bytes()))))
