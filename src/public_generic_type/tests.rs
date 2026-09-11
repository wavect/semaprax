//! Executable evidence for gates PG-1 and PG-2 of the Public Generic
//! Ownership milestone: the grammar's vocabulary is closed, its terms are
//! injective and target-neutral, its identities are positional rather than
//! nominal, and hostile bytes fail closed instead of being repaired.

use std::path::Path;

use super::*;
use crate::hir::{self, DeclarationId, ResolvedProgram, ResolvedType};
use crate::parse;

const SOURCE: &str = r#"
module test.public_generic_grammar;

@id("grammar.pair")
record Pair<T, U> {
    @id("grammar.pair.left")
    left: T,
    @id("grammar.pair.right")
    right: U,
}

@id("grammar.plain")
record Plain {
    @id("grammar.plain.payload")
    payload: Bytes,
}

@id("grammar.choice")
variant Choice {
    @id("grammar.choice.none")
    None {},
}

@id("grammar.use")
fn use_declarations(input: borrow Slice<u8>) -> i64 {
    let pair = Pair<Bytes, bool> {
        left: bytes_copy(input),
        right: byte_len(input) > 0usize,
    };
    let plain = Plain { payload: bytes_copy(input) };
    match own pair {
        Pair { left: bytes, right: flag } =>
            match own plain {
                Plain { payload: more } =>
                    if flag && byte_len(bytes_as_slice(bytes)) > 0usize
                        && byte_len(bytes_as_slice(more)) > 0usize { 1 } else { 0 },
            },
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn program() -> ResolvedProgram {
    let parsed = parse(SOURCE, Path::new("grammar.spx")).unwrap();
    hir::resolve(&parsed).unwrap()
}

fn nominal(declaration: &str, arguments: Vec<ResolvedType>) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(declaration),
        arguments,
    }
}

fn pair(arguments: Vec<ResolvedType>) -> ResolvedType {
    nominal("grammar.pair", arguments)
}

/// The identity prefix counts bytes, so the expected term is written out in
/// full rather than assembled by the same code that produces it.
#[test]
fn a_flat_instance_renders_one_exact_target_neutral_term() {
    let program = program();
    let rendered = term(
        &program,
        &pair(vec![ResolvedType::Bytes, ResolvedType::Bool]),
    )
    .unwrap();
    assert_eq!(rendered, "@12:grammar.pair<bytes,bool>");
    assert_eq!(parse_term(&rendered).unwrap().render(), rendered);
    for target_fact in [
        "int64_t",
        "i64.const",
        "uint8_t",
        "size_of",
        "align",
        "layout",
        "ptr",
        "memory",
    ] {
        assert!(
            !rendered.contains(target_fact),
            "a target-neutral term must not mention {target_fact}"
        );
    }
}

/// A concrete record with no type parameters still renders as an instance with
/// an empty ordered argument vector.
#[test]
fn a_zero_arity_record_renders_empty_ordered_arguments() {
    let program = program();
    let facts = describe(&program, &nominal("grammar.plain", Vec::new())).unwrap();
    assert_eq!(facts.term, "@13:grammar.plain<>");
    assert_eq!(facts.template.arity, 0);
    assert!(facts.arguments.is_empty());
    assert_eq!(facts.owned_leaves, vec!["@21:grammar.plain.payload"]);
}

/// Nesting substitutes owner-and-index before descending, and owned leaves
/// keep structural order with identity-framed paths.
#[test]
fn nested_instances_substitute_before_descending_and_order_owned_leaves() {
    let program = program();
    let inner = pair(vec![ResolvedType::Bytes, ResolvedType::Bool]);
    let facts = describe(&program, &pair(vec![inner.clone(), ResolvedType::Bytes])).unwrap();
    assert_eq!(
        facts.term,
        "@12:grammar.pair<@12:grammar.pair<bytes,bool>,bytes>"
    );
    assert_eq!(
        facts.owned_leaves,
        vec![
            "@17:grammar.pair.left/@17:grammar.pair.left",
            "@18:grammar.pair.right",
        ]
    );
    let fields = facts
        .fields
        .iter()
        .map(|field| (field.index, field.id.as_str(), field.term.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        fields,
        vec![
            (0, "grammar.pair.left", "@12:grammar.pair<bytes,bool>"),
            (1, "grammar.pair.right", "bytes"),
        ]
    );
    assert_eq!(parse_term(&facts.term).unwrap().render(), facts.term);
}

/// Identity, not presentation. Renaming the record and its fields leaves every
/// digest unchanged; the milestone's whole point is that a display rename
/// cannot move a foreign calling convention.
#[test]
fn display_renames_do_not_change_any_identity() {
    let program = program();
    let instance = pair(vec![ResolvedType::Bytes, ResolvedType::Bool]);
    let before = describe(&program, &instance).unwrap();

    let renamed_source = SOURCE
        .replace("record Pair<T, U>", "record Couple<A, B>")
        .replace("    left: T,", "    first: A,")
        .replace("    right: U,", "    second: B,")
        .replace("Pair<Bytes, bool> {", "Couple<Bytes, bool> {")
        .replace(
            "        left: bytes_copy(input),",
            "        first: bytes_copy(input),",
        )
        .replace(
            "        right: byte_len(input) > 0usize,",
            "        second: byte_len(input) > 0usize,",
        )
        .replace(
            "        Pair { left: bytes, right: flag } =>",
            "        Couple { first: bytes, second: flag } =>",
        );
    let parsed = parse(&renamed_source, Path::new("renamed.spx")).unwrap();
    let renamed = hir::resolve(&parsed).unwrap();
    let after = describe(&renamed, &instance).unwrap();

    assert_eq!(after.template.name, "Couple");
    assert_eq!(before.template.name, "Pair");
    assert_eq!(after.template.digest, before.template.digest);
    assert_eq!(after.instance_digest, before.instance_digest);
    assert_eq!(after.term, before.term);
    assert_eq!(
        after
            .fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "second"]
    );
}

/// Ordered arguments are ordered. Permuting, duplicating, or substituting one
/// changes the instance digest; omitting one is not a different instance but a
/// closed rejection.
#[test]
fn argument_order_and_content_are_part_of_the_instance_identity() {
    let program = program();
    let baseline = describe(
        &program,
        &pair(vec![ResolvedType::Bytes, ResolvedType::Bool]),
    )
    .unwrap();
    let permuted = describe(
        &program,
        &pair(vec![ResolvedType::Bool, ResolvedType::Bytes]),
    )
    .unwrap();
    let duplicated = describe(
        &program,
        &pair(vec![ResolvedType::Bytes, ResolvedType::Bytes]),
    )
    .unwrap();
    let substituted = describe(
        &program,
        &pair(vec![ResolvedType::Bytes, ResolvedType::I64]),
    )
    .unwrap();

    let digests = [
        &baseline.instance_digest,
        &permuted.instance_digest,
        &duplicated.instance_digest,
        &substituted.instance_digest,
    ];
    for (left, right) in [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)] {
        assert_ne!(
            digests[left], digests[right],
            "argument vectors {left} and {right} must not share an identity"
        );
    }
    // The template is the same template in every case.
    for facts in [&permuted, &duplicated, &substituted] {
        assert_eq!(facts.template.digest, baseline.template.digest);
    }
    assert_eq!(
        describe(&program, &pair(vec![ResolvedType::Bytes]))
            .unwrap_err()
            .code,
        REJECTED_TYPE
    );
}

/// Every closed rejection reason is reachable, and each one is the reason the
/// grammar reports. A reason that cannot be produced is not a closed
/// vocabulary; a type that is admitted by accident is worse.
#[test]
fn every_closed_rejection_reason_is_reachable() {
    let mut program = program();
    let cases: Vec<(Rejection, ResolvedType)> = vec![
        (
            Rejection::TypeParameter,
            ResolvedType::TypeParameter {
                owner: DeclarationId::new("grammar.pair"),
                index: 0,
            },
        ),
        (Rejection::OwnedString, ResolvedType::String),
        (Rejection::BorrowedStr, ResolvedType::Str),
        (Rejection::BorrowedByteView, ResolvedType::SliceU8),
        (Rejection::Unit, ResolvedType::Unit),
        (Rejection::InlineByteArray, ResolvedType::ArrayU8(4)),
        (
            Rejection::FunctionType,
            ResolvedType::Function {
                parameters: vec![ResolvedType::I64],
                result: Box::new(ResolvedType::I64),
            },
        ),
        (
            Rejection::CompilerOwnedNominal,
            nominal(crate::prelude::OPTION_ID, vec![ResolvedType::I64]),
        ),
        (
            Rejection::UnadmittedNominalKind,
            nominal("grammar.choice", Vec::new()),
        ),
        (
            Rejection::MissingDeclaration,
            nominal("grammar.absent", Vec::new()),
        ),
        (
            Rejection::ArityMismatch,
            pair(vec![
                ResolvedType::Bytes,
                ResolvedType::Bool,
                ResolvedType::I64,
            ]),
        ),
    ];
    for (rejection, ty) in cases {
        let error = classify(&program, &ty)
            .expect_err(&format!("{} must reject", rejection.reason()))
            .message;
        assert!(
            error.ends_with(rejection.reason()),
            "expected {}, got {error}",
            rejection.reason()
        );
    }

    // Ambiguity is a property of the program, not of one type, so it needs a
    // program whose inventory really does carry the identity twice.
    let duplicate = program
        .types
        .iter()
        .find(|declaration| declaration.id.as_str() == "grammar.pair")
        .cloned()
        .unwrap();
    program.types.push(duplicate);
    let error = classify(
        &program,
        &pair(vec![ResolvedType::Bytes, ResolvedType::Bool]),
    )
    .expect_err("a repeated declaration identity must fail closed")
    .message;
    assert!(
        error.ends_with(Rejection::AmbiguousDeclaration.reason()),
        "got {error}"
    );
}

/// The length prefix is what makes the grammar injective: an identity holding
/// the grammar's own punctuation still renders and parses back exactly.
#[test]
fn identities_containing_grammar_punctuation_round_trip() {
    for identity in [
        "",
        "a<b>",
        "a,b",
        "a:b",
        "@9:not-a-prefix",
        "bytes",
        "i64",
        "nön-ascii-ß",
        "<>,:@",
    ] {
        let rendered = GrammarTerm::Instance {
            declaration: identity.to_owned(),
            arguments: vec![GrammarTerm::Bytes],
        }
        .render();
        let parsed = parse_term(&rendered).unwrap();
        assert_eq!(
            parsed,
            GrammarTerm::Instance {
                declaration: identity.to_owned(),
                arguments: vec![GrammarTerm::Bytes],
            },
            "{identity:?} did not round trip through {rendered}"
        );
        assert_eq!(parsed.render(), rendered);
    }
}

/// Two distinct identities never render alike, including the pair that a
/// delimiter-only grammar would confuse.
#[test]
fn distinct_identities_never_render_alike() {
    let left = GrammarTerm::Instance {
        declaration: "a".to_owned(),
        arguments: vec![GrammarTerm::Instance {
            declaration: "b".to_owned(),
            arguments: Vec::new(),
        }],
    };
    let right = GrammarTerm::Instance {
        declaration: "a<@1:b<>".to_owned(),
        arguments: Vec::new(),
    };
    assert_ne!(left.render(), right.render());
    assert_ne!(term_digest(&left.render()), term_digest(&right.render()));
}

/// Hostile or merely sloppy bytes fail closed. None of these is repaired into
/// a neighbouring valid term.
#[test]
fn malformed_terms_fail_closed() {
    let cases = [
        ("", "empty"),
        (" bytes", "leading space"),
        ("bytes ", "trailing space"),
        ("Bytes", "wrong case"),
        ("bytes,bool", "two terms"),
        ("@", "prefix only"),
        ("@4", "no colon"),
        ("@04:abcd<>", "leading zero length"),
        ("@:abcd<>", "empty length"),
        ("@x:abcd<>", "non-decimal length"),
        ("@5:abcd<>", "length longer than identity"),
        ("@4:abcd", "no argument list"),
        ("@4:abcd<", "unclosed argument list"),
        ("@4:abcd<bytes", "unterminated arguments"),
        ("@4:abcd<bytes,>", "trailing comma"),
        ("@4:abcd<,bytes>", "leading comma"),
        ("@4:abcd<bytes>>", "trailing bytes"),
        ("@4:abcd<bytes bool>", "space separated arguments"),
        ("@1:ß<>", "length splits a UTF-8 sequence"),
        ("@3:abc<unknown>", "unknown token"),
    ];
    for (input, label) in cases {
        let error =
            parse_term(input).expect_err(&format!("{label} ({input:?}) must fail to parse"));
        assert!(
            matches!(error.code, MALFORMED_TERM | GRAMMAR_CAPACITY),
            "{label} rejected with {}",
            error.code
        );
    }
    let nul = format!("@3:a{}c<>", '\0');
    assert_eq!(parse_term(&nul).unwrap_err().code, MALFORMED_TERM);
}

/// Bounds are refusals, not truncations.
#[test]
fn grammar_bounds_refuse_rather_than_truncate() {
    let mut deep = String::new();
    for _ in 0..=MAX_RECORD_DEPTH {
        deep.push_str("@1:a<");
    }
    deep.push_str("bytes");
    for _ in 0..=MAX_RECORD_DEPTH {
        deep.push('>');
    }
    assert_eq!(parse_term(&deep).unwrap_err().code, GRAMMAR_CAPACITY);

    let wide = format!(
        "@1:a<{}>",
        std::iter::repeat_n("bytes", MAX_TEMPLATE_ARITY + 1)
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_eq!(parse_term(&wide).unwrap_err().code, GRAMMAR_CAPACITY);

    let oversized = format!(
        "@{}:{}<>",
        MAX_TERM_BYTES + 1,
        "a".repeat(MAX_TERM_BYTES + 1)
    );
    assert_eq!(parse_term(&oversized).unwrap_err().code, GRAMMAR_CAPACITY);
}

/// Replay is byte-exact against an independent recomputation, and submitted
/// bytes are never taken as the answer.
#[test]
fn replay_requires_byte_equality_with_the_recomputed_term() {
    let program = program();
    let instance = pair(vec![ResolvedType::Bytes, ResolvedType::Bool]);
    let canonical = term(&program, &instance).unwrap();
    verify_term(&program, &instance, &canonical).unwrap();

    // A term of a different, equally valid instance must not verify.
    let other = term(
        &program,
        &pair(vec![ResolvedType::Bool, ResolvedType::Bytes]),
    )
    .unwrap();
    assert_eq!(
        verify_term(&program, &instance, &other).unwrap_err().code,
        TERM_REPLAY_MISMATCH
    );
    assert_eq!(
        verify_term(&program, &instance, "@12:grammar.pair<bytes,i64>")
            .unwrap_err()
            .code,
        TERM_REPLAY_MISMATCH
    );
    assert_eq!(
        verify_term(&program, &instance, "bytes").unwrap_err().code,
        TERM_REPLAY_MISMATCH
    );
    assert_eq!(
        verify_term(&program, &instance, "@12:grammar.pair<bytes,bool")
            .unwrap_err()
            .code,
        MALFORMED_TERM
    );
}

/// Determinism: the same checked program and type produce identical facts,
/// including every digest, on repeated projection.
#[test]
fn description_is_deterministic() {
    let program = program();
    let instance = pair(vec![
        pair(vec![ResolvedType::Bytes, ResolvedType::Bool]),
        ResolvedType::Bytes,
    ]);
    let first = describe(&program, &instance).unwrap();
    let second = describe(&program, &instance).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.term_digest, term_digest(&first.term));
}

/// The digests are domain-separated: term, template, and instance preimages
/// cannot collide even when their framed content would otherwise agree.
#[test]
fn digest_domains_are_separated() {
    let program = program();
    let facts = describe(
        &program,
        &pair(vec![ResolvedType::Bytes, ResolvedType::Bool]),
    )
    .unwrap();
    assert_ne!(facts.term_digest, facts.template.digest);
    assert_ne!(facts.term_digest, facts.instance_digest);
    assert_ne!(facts.template.digest, facts.instance_digest);
    for digest in [
        &facts.term_digest,
        &facts.template.digest,
        &facts.instance_digest,
    ] {
        assert!(digest.starts_with("sha256:"));
        assert_eq!(digest.len(), 71);
    }
}

/// Ordered parameter positions are bound to their owner, so two templates that
/// declare the same arity never share a template identity.
#[test]
fn template_identities_bind_owner_and_position() {
    let program = program();
    let pair_facts = describe(
        &program,
        &pair(vec![ResolvedType::Bytes, ResolvedType::Bool]),
    )
    .unwrap();
    let plain_facts = describe(&program, &nominal("grammar.plain", Vec::new())).unwrap();
    assert_ne!(pair_facts.template.digest, plain_facts.template.digest);
    assert_eq!(
        pair_facts
            .template
            .parameters
            .iter()
            .map(|parameter| (parameter.owner.as_str(), parameter.index))
            .collect::<Vec<_>>(),
        vec![("grammar.pair", 0), ("grammar.pair", 1)]
    );
    assert_eq!(
        pair_facts
            .arguments
            .iter()
            .map(|argument| (
                argument.index,
                argument.parameter_index,
                argument.term.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![(0, 0, "bytes"), (1, 1, "bool")]
    );
}
