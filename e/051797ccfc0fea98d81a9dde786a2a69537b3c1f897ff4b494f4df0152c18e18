//! Exact main invoice for its retained executable comparator.
use super::*;

#[test]
fn comparison_census_covers_every_actual_owner_payment_and_lexical_forwarding_site() {
    // Closed current invocation grammar. An unfamiliar source shape fails this
    // guard instead of silently omitting a real payment from the census.
    fn phases<'a>(source: &'a str, token: &str, arguments: &[usize]) -> Vec<(&'a str, usize)> {
        let mut phases = Vec::new();
        for (start, _) in source.match_indices(token) {
            let begin = start + token.len();
            let bytes = source.as_bytes();
            let mut at = begin;
            let mut depth = 1usize;
            let mut quoted = false;
            let mut escaped = false;
            while at < bytes.len() && depth != 0 {
                let byte = bytes[at];
                at += 1;
                if quoted {
                    if escaped {
                        escaped = false;
                    } else if byte == b'\\' {
                        escaped = true;
                    } else if byte == b'"' {
                        quoted = false;
                    }
                } else if byte == b'"' {
                    quoted = true;
                } else if byte == b'(' {
                    depth += 1;
                } else if byte == b')' {
                    depth -= 1;
                }
            }
            assert_eq!(depth, 0, "unterminated payment invocation");
            let body = &source[begin..at - 1];
            assert!(!body.contains("//") && !body.contains("/*") && !body.contains('\\'));
            assert_eq!(
                body.matches('"').count(),
                2,
                "unknown payment argument shape"
            );
            let first = body.find('"').unwrap();
            let last = body.rfind('"').unwrap();
            let arity = body[..first].matches(',').count() + 1;
            assert!(arguments.contains(&arity), "unknown macro arity");
            assert!(body[last + 1..].trim().trim_matches(',').trim().is_empty());
            phases.push((&body[first + 1..last], arity));
        }
        phases
    }
    let owner = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/mod.rs"
    ));
    let direct = phases(owner, "paid!(", &[4]);
    let forwarded = phases(owner, "lexical_plan_next!(", &[4, 10, 12]);
    let shared = phases(owner, "shared_plan_pair_step!(", &[12]);
    assert_eq!(
        (direct.len(), forwarded.len(), shared.len()),
        (12, 4, 1),
        "audit new lexical payment sites explicitly"
    );
    assert_eq!(forwarded.iter().filter(|(_, n)| *n == 10).count(), 2);
    assert_eq!(forwarded.iter().filter(|(_, n)| *n == 4).count(), 1);
    assert_eq!(forwarded.iter().filter(|(_, n)| *n == 12).count(), 1);
    let lexical = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/lexical_cursor.rs"
    ));
    let sites: Vec<_> = lexical.match_indices("paid!(").collect();
    assert_eq!(sites.len(), 27, "audit every generic payment site");
    assert_eq!(lexical.matches("shared_plan_boundary!(").count(), 1);
    assert_eq!(lexical.matches("shared_plan_segment_run!(").count(), 1);
    assert_eq!(lexical.matches("independent_plan_segment_run!(").count(), 1);
    assert_eq!(lexical.matches("paired_plan_tails!(").count(), 1);
    assert_eq!(lexical.matches("remember_plan_tail!(").count(), 3);
    let compact: String = lexical.split_whitespace().collect();
    assert!(!compact.contains("lexical_pair_plan_next!"));
    assert_eq!(compact.matches("paired_plan_observations!($owner,$left_path,$right_path,$left_tail,$right_tail,$left_links,$right_links,$census,$function,$budget,$pair_loop,$phase);").count(), 1);
    assert_eq!(
        compact
            .matches("shared_plan_boundary!($owner,$left_links,$right_links,$census,$function,$budget,$phase);")
            .count(),
        1
    );
    assert_eq!(
        compact
            .matches("shared_plan_segment_run!($owner,$census,$function,$budget,$phase);")
            .count(),
        1
    );
    assert_eq!(compact.matches("independent_plan_segment_run!($owner,$census,$function,$budget,$pair_loop,$phase);").count(), 1);
    for (at, _) in sites {
        let body = &lexical[at + "paid!(".len()..];
        let end = body.find(")?;").expect("unknown generic payment shape");
        let phase = body[..end].trim().trim_end_matches(',').trim();
        assert!(
            phase
                .rsplit_once(',')
                .is_some_and(|(_, last)| last.trim() == "$phase"),
            "unknown generic payment forwarding shape: {phase}"
        );
    }
    for (phase, _) in direct.into_iter().chain(forwarded).chain(shared) {
        assert!(
            comparison_phase(phase).is_some(),
            "unattributed actual payment phase: {phase}"
        );
    }
}
