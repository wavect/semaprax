//! The per-obligation assurance lattice: a closed class vocabulary and its
//! **partial** dominance order.
//!
//! See [`docs/ASSURANCE-MANIFEST-V1.md`](../../docs/ASSURANCE-MANIFEST-V1.md)
//! "The assurance lattice" for the rationale behind every edge this module
//! does and does not assert. `AssuranceClass`'s derived `Ord` fixes only the
//! declaration order used by [`AssuranceClass::ALL`] and iteration; it is
//! **not** the lattice's dominance relation, which [`dominates`] computes
//! separately from [`DIRECT_EDGES`] and is deliberately incomplete (several
//! pairs are incomparable on purpose).

/// Closed assurance-class vocabulary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AssuranceClass {
    Open,
    Assumed,
    AttemptInconclusive,
    TestEvidenced,
    RuntimeGuarded,
    CompilerProved,
    ModelChecked,
    SmtProved,
    TheoremProved,
}

impl AssuranceClass {
    pub const ALL: [Self; 9] = [
        Self::Open,
        Self::Assumed,
        Self::AttemptInconclusive,
        Self::TestEvidenced,
        Self::RuntimeGuarded,
        Self::CompilerProved,
        Self::ModelChecked,
        Self::SmtProved,
        Self::TheoremProved,
    ];

    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Assumed => "assumed",
            Self::AttemptInconclusive => "attempt_inconclusive",
            Self::TestEvidenced => "test_evidenced",
            Self::RuntimeGuarded => "runtime_guarded",
            Self::CompilerProved => "compiler_proved",
            Self::ModelChecked => "model_checked",
            Self::SmtProved => "smt_proved",
            Self::TheoremProved => "theorem_proved",
        }
    }

    /// Parse one exact class token. Unknown or case-folded names are
    /// rejected; a forged or unrecognized token in an envelope is a replay
    /// failure (`SPX-Z103`), never accepted silently.
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|class| class.token() == token)
    }

    /// Fixed alphabetical rank over [`Self::token`], used **only** to break
    /// ties among an obligation's Pareto-maximal method-record classes in
    /// [`classification_of`]. This is a rendering tie-break, not an
    /// assurance claim: it never implies dominance the lattice does not
    /// assert.
    #[must_use]
    const fn rank_for_display(self) -> u8 {
        match self {
            Self::Assumed => 0,
            Self::AttemptInconclusive => 1,
            Self::CompilerProved => 2,
            Self::ModelChecked => 3,
            Self::Open => 4,
            Self::RuntimeGuarded => 5,
            Self::SmtProved => 6,
            Self::TestEvidenced => 7,
            Self::TheoremProved => 8,
        }
    }
}

/// The hand-reviewed, minimal direct-edge set the whole dominance relation
/// is the reachability closure of. Every edge here is individually
/// justified in the owning specification; anything not reachable through
/// this table is deliberately incomparable, never assumed dominant.
pub(super) const DIRECT_EDGES: &[(AssuranceClass, AssuranceClass)] = &[
    (AssuranceClass::TheoremProved, AssuranceClass::SmtProved),
    (AssuranceClass::TheoremProved, AssuranceClass::ModelChecked),
    (
        AssuranceClass::TheoremProved,
        AssuranceClass::CompilerProved,
    ),
    (
        AssuranceClass::TheoremProved,
        AssuranceClass::RuntimeGuarded,
    ),
    (AssuranceClass::TheoremProved, AssuranceClass::TestEvidenced),
    (AssuranceClass::SmtProved, AssuranceClass::ModelChecked),
    (AssuranceClass::SmtProved, AssuranceClass::RuntimeGuarded),
    (AssuranceClass::SmtProved, AssuranceClass::TestEvidenced),
    (AssuranceClass::ModelChecked, AssuranceClass::TestEvidenced),
    (
        AssuranceClass::CompilerProved,
        AssuranceClass::RuntimeGuarded,
    ),
    (
        AssuranceClass::CompilerProved,
        AssuranceClass::TestEvidenced,
    ),
    (
        AssuranceClass::RuntimeGuarded,
        AssuranceClass::TestEvidenced,
    ),
    (
        AssuranceClass::TestEvidenced,
        AssuranceClass::AttemptInconclusive,
    ),
    (AssuranceClass::AttemptInconclusive, AssuranceClass::Assumed),
    (AssuranceClass::Assumed, AssuranceClass::Open),
];

/// `true` when `a` dominates `b`: `a == b`, or `b` is reachable from `a`
/// through [`DIRECT_EDGES`]. This is a **partial** order: for many pairs
/// (for example `compiler_proved` and `smt_proved`) neither
/// `dominates(a, b)` nor `dominates(b, a)` holds, by design.
#[must_use]
pub fn dominates(a: AssuranceClass, b: AssuranceClass) -> bool {
    if a == b {
        return true;
    }
    let mut frontier = vec![a];
    let mut visited = vec![a];
    while let Some(current) = frontier.pop() {
        for &(from, to) in DIRECT_EDGES {
            if from == current {
                if to == b {
                    return true;
                }
                if !visited.contains(&to) {
                    visited.push(to);
                    frontier.push(to);
                }
            }
        }
    }
    false
}

/// One obligation's single current classification, deterministically
/// derived from its method records' classes without asserting a dominance
/// the lattice does not support.
///
/// An obligation with no method records is `open` (nothing attempted yet).
/// Otherwise: if exactly one class among the records dominates every other
/// class present, that class is returned. If several classes are mutually
/// incomparable maxima (the Pareto frontier), the one with the lowest
/// [`AssuranceClass::rank_for_display`] is returned — a fixed, documented
/// tie-break, never a claim that it outranks its siblings.
#[must_use]
pub fn classification_of(classes: &[AssuranceClass]) -> AssuranceClass {
    if classes.is_empty() {
        return AssuranceClass::Open;
    }
    let mut present: Vec<AssuranceClass> = Vec::new();
    for &class in classes {
        if !present.contains(&class) {
            present.push(class);
        }
    }
    let frontier: Vec<AssuranceClass> = present
        .iter()
        .copied()
        .filter(|&candidate| {
            present
                .iter()
                .all(|&other| other == candidate || !dominates(other, candidate))
        })
        .collect();
    let mut winner = frontier[0];
    for &candidate in &frontier[1..] {
        if candidate.rank_for_display() < winner.rank_for_display() {
            winner = candidate;
        }
    }
    winner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_dominates_itself() {
        for class in AssuranceClass::ALL {
            assert!(dominates(class, class));
        }
    }

    #[test]
    fn dominance_is_irreflexive_off_diagonal_or_antisymmetric() {
        // A strict partial order never has both a>b and b>a for a != b.
        for a in AssuranceClass::ALL {
            for b in AssuranceClass::ALL {
                if a != b {
                    assert!(
                        !(dominates(a, b) && dominates(b, a)),
                        "{a:?} and {b:?} dominate each other"
                    );
                }
            }
        }
    }

    #[test]
    fn dominance_is_transitive() {
        for a in AssuranceClass::ALL {
            for b in AssuranceClass::ALL {
                for c in AssuranceClass::ALL {
                    if dominates(a, b) && dominates(b, c) {
                        assert!(
                            dominates(a, c),
                            "{a:?} > {b:?} > {c:?} but not {a:?} > {c:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn open_is_the_unique_bottom() {
        for class in AssuranceClass::ALL {
            if class != AssuranceClass::Open {
                assert!(dominates(class, AssuranceClass::Open));
            }
        }
        for class in AssuranceClass::ALL {
            if class != AssuranceClass::Open {
                assert!(!dominates(AssuranceClass::Open, class));
            }
        }
    }

    #[test]
    fn theorem_proved_is_the_unique_top() {
        for class in AssuranceClass::ALL {
            if class != AssuranceClass::TheoremProved {
                assert!(dominates(AssuranceClass::TheoremProved, class));
                assert!(!dominates(class, AssuranceClass::TheoremProved));
            }
        }
    }

    #[test]
    fn compiler_proved_and_smt_proved_are_deliberately_incomparable() {
        assert!(!dominates(
            AssuranceClass::CompilerProved,
            AssuranceClass::SmtProved
        ));
        assert!(!dominates(
            AssuranceClass::SmtProved,
            AssuranceClass::CompilerProved
        ));
    }

    #[test]
    fn compiler_proved_and_model_checked_are_deliberately_incomparable() {
        assert!(!dominates(
            AssuranceClass::CompilerProved,
            AssuranceClass::ModelChecked
        ));
        assert!(!dominates(
            AssuranceClass::ModelChecked,
            AssuranceClass::CompilerProved
        ));
    }

    #[test]
    fn test_evidenced_never_dominates_a_stronger_method() {
        for class in AssuranceClass::ALL {
            if class != AssuranceClass::TestEvidenced
                && class != AssuranceClass::AttemptInconclusive
                && class != AssuranceClass::Assumed
                && class != AssuranceClass::Open
            {
                assert!(!dominates(AssuranceClass::TestEvidenced, class));
            }
        }
    }

    #[test]
    fn classification_of_empty_is_open() {
        assert_eq!(classification_of(&[]), AssuranceClass::Open);
    }

    #[test]
    fn classification_of_single_record_is_itself() {
        assert_eq!(
            classification_of(&[AssuranceClass::RuntimeGuarded]),
            AssuranceClass::RuntimeGuarded
        );
    }

    #[test]
    fn classification_of_picks_the_unique_dominant_record() {
        assert_eq!(
            classification_of(&[
                AssuranceClass::TestEvidenced,
                AssuranceClass::RuntimeGuarded,
                AssuranceClass::AttemptInconclusive,
            ]),
            AssuranceClass::RuntimeGuarded
        );
    }

    #[test]
    fn classification_of_ties_incomparable_maxima_deterministically() {
        // compiler_proved and smt_proved are incomparable, so both are
        // Pareto-maximal here; the tie-break must be deterministic and
        // documented, not an implied dominance.
        let first = classification_of(&[AssuranceClass::CompilerProved, AssuranceClass::SmtProved]);
        let second =
            classification_of(&[AssuranceClass::SmtProved, AssuranceClass::CompilerProved]);
        assert_eq!(first, second);
        assert_eq!(first, AssuranceClass::CompilerProved);
    }

    #[test]
    fn classification_of_never_reports_a_class_absent_from_its_records() {
        for a in AssuranceClass::ALL {
            for b in AssuranceClass::ALL {
                let winner = classification_of(&[a, b]);
                assert!(winner == a || winner == b);
            }
        }
    }
}
