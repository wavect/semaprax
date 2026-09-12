//! Executable evidence for [`classify`]: every positive case is a real
//! program compiled through `crate::parse` + `crate::hir::resolve`, exactly
//! as [`crate::public_generic_type`], [`crate::public_generic_surface`], and
//! [`crate::public_generic_abi::descriptor::producer`] already require of
//! their own fixtures. Several negative cases probe an internal-consistency
//! condition the front end itself already prevents (an ambiguous stable
//! identity, a cyclic record graph, an unresolved type argument reaching a
//! monomorphic signature) — for those, a real compiled program is cloned and
//! then adversarially mutated at the checked-HIR level, exactly the
//! technique [`crate::public_generic_type::tests`] already uses to reach
//! [`grammar::Rejection::ArityMismatch`] and friends. Every mutation is
//! called out at its test.

use std::path::Path;

use super::*;
use crate::ast::Span;
use crate::hir::{
    self, DeclarationId, ResolvedFieldDeclaration, ResolvedProgram, ResolvedTypeDeclaration,
    ResolvedTypeDeclarationKind,
};
use crate::parse;

/// One base program exercising: a nested owned record (`Pair<Leaf, i64>`),
/// a scalar value-mode parameter, an owned non-record parameter, a borrowed
/// top-level parameter, two parameters, a non-owned result, a declared
/// effect, and a generic function template — one clean compiled export per
/// export-shape refusal this classifier distinguishes.
const BASE: &str = r#"
module test.public_generic_classifier;

permit { clock.read }

@id("classifier.leaf")
record Leaf {
    @id("classifier.leaf.head")
    head: Bytes,
}

@id("classifier.pair")
record Pair<T, U> {
    @id("classifier.pair.left")
    left: T,
    @id("classifier.pair.right")
    right: U,
}

@id("classifier.status")
variant Status {
    @id("classifier.status.ok")
    Ok {},
}

@id("classifier.status_maker")
fn status_maker() -> Status { Status::Ok {} }

@id("classifier.scalars")
record Scalars {
    @id("classifier.scalars.a")
    a: i64,
    @id("classifier.scalars.b")
    b: i32,
    @id("classifier.scalars.c")
    c: u8,
    @id("classifier.scalars.d")
    d: usize,
    @id("classifier.scalars.e")
    e: char,
    @id("classifier.scalars.f")
    f: f32,
    @id("classifier.scalars.g")
    g: f64,
    @id("classifier.scalars.h")
    h: bool,
    @id("classifier.scalars.leaf")
    leaf: Bytes,
}

@id("classifier.take")
fn take(value: own Pair<Leaf, i64>) -> Pair<Leaf, i64> { value }

@id("classifier.scalars_take")
fn scalars_take(value: own Scalars) -> Scalars { value }

@id("classifier.scalar")
fn scalar(value: i64) -> i64 { value }

@id("classifier.owned_bytes")
fn owned_bytes(value: own Bytes) -> Bytes { value }

@id("classifier.borrowed")
fn borrowed(value: borrow Pair<Leaf, i64>) -> i64 {
    match borrow value {
        Pair { left: Leaf { head: _head }, right: count } => count,
    }
}

@id("classifier.two_params")
fn two_params(value: own Pair<Leaf, i64>, extra: i64) -> Pair<Leaf, i64> { value }

@id("classifier.no_owned_result")
fn no_owned_result(value: own Pair<Leaf, i64>) -> i64 {
    match own value {
        Pair { left: Leaf { head: _head }, right: count } => count,
    }
}

@id("classifier.effectful")
fn effectful(value: own Pair<Leaf, i64>) -> Pair<Leaf, i64>
    uses { clock.read }
{
    value
}

@id("classifier.identity")
fn identity<T>(value: own Pair<Leaf, T>) -> Pair<Leaf, T> { value }

@id("classifier.use_identity")
fn use_identity(value: own Pair<Leaf, i64>) -> Pair<Leaf, i64> { identity<i64>(value) }

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// Every display name changed; every persistent identity, declaration set,
/// ownership mode, and structural shape kept exactly the same as [`BASE`]'s
/// `take` export and its reachable closure.
const RENAMED: &str = r#"
module test.public_generic_classifier_renamed;

@id("classifier.leaf")
record Node {
    @id("classifier.leaf.head")
    contents: Bytes,
}

@id("classifier.pair")
record Couple<A, B> {
    @id("classifier.pair.left")
    first: A,
    @id("classifier.pair.right")
    second: B,
}

@id("classifier.take")
fn accept(subject: own Couple<Node, i64>) -> Couple<Node, i64> { subject }

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// `Pair<Leaf, i64>` in, `Leaf` out: two different templates across input
/// and result.
const DIFFERENT_TEMPLATES: &str = r#"
module test.public_generic_classifier_different_templates;

@id("classifier.leaf")
record Leaf {
    @id("classifier.leaf.head")
    head: Bytes,
}

@id("classifier.pair")
record Pair<T, U> {
    @id("classifier.pair.left")
    left: T,
    @id("classifier.pair.right")
    right: U,
}

@id("classifier.split")
fn split(value: own Pair<Leaf, i64>) -> Leaf {
    match own value {
        Pair { left: Leaf { head: payload }, right: _count } => Leaf { head: payload },
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// Same template (`Pair`), different concrete arguments for input and
/// result: `Pair<Leaf, i64>` in, `Pair<Leaf, bool>` out.
const SAME_TEMPLATE_DIFFERENT_ARGUMENTS: &str = r#"
module test.public_generic_classifier_same_template;

@id("classifier.leaf")
record Leaf {
    @id("classifier.leaf.head")
    head: Bytes,
}

@id("classifier.pair")
record Pair<T, U> {
    @id("classifier.pair.left")
    left: T,
    @id("classifier.pair.right")
    right: U,
}

@id("classifier.recount")
fn recount(value: own Pair<Leaf, i64>) -> Pair<Leaf, bool> {
    match own value {
        Pair { left: Leaf { head: payload }, right: count } =>
            Pair<Leaf, bool> { left: Leaf { head: payload }, right: count > 0 },
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// `Pair<i64, Leaf>`: the same template and the same two concrete arguments
/// as `BASE`'s `classifier.take` (`Pair<Leaf, i64>`), positionally swapped.
/// Used only to prove a same-count argument reorder is admitted as a
/// distinct instance rather than conflated with `ArityMismatch`.
const SWAPPED_ARGUMENTS: &str = r#"
module test.public_generic_classifier_swapped_arguments;

@id("classifier.leaf")
record Leaf {
    @id("classifier.leaf.head")
    head: Bytes,
}

@id("classifier.pair")
record Pair<T, U> {
    @id("classifier.pair.left")
    left: T,
    @id("classifier.pair.right")
    right: U,
}

@id("classifier.swap_take")
fn swap_take(value: own Pair<i64, Leaf>) -> Pair<i64, Leaf> { value }

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn resolved(source: &str) -> ResolvedProgram {
    let parsed = parse(source, Path::new("classifier.spx")).unwrap();
    hir::resolve(&parsed).unwrap()
}

fn nominal(declaration: &str, arguments: Vec<ResolvedType>) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(declaration),
        arguments,
    }
}

/// Overwrite `classifier.leaf`'s only field (`head`) with `ty`, on a clone
/// of an already fully checked program. This is the same "compile once,
/// then adversarially mutate one checked fact" technique
/// `public_generic_type::tests` uses; the mutated value could never come
/// from a real monomorphic checked program on its own, which is exactly why
/// it belongs here instead of in a `.spx` fixture.
fn set_leaf_head_type(program: &mut ResolvedProgram, ty: ResolvedType) {
    let index = program
        .types
        .iter()
        .position(|declaration| declaration.id.as_str() == "classifier.leaf")
        .expect("BASE declares classifier.leaf");
    let ResolvedTypeDeclarationKind::Record { fields } = &mut program.types[index].kind else {
        panic!("classifier.leaf is a record declaration");
    };
    fields[0].ty = ty;
}

fn set_parameter_type(program: &mut ResolvedProgram, export_id: &str, ty: ResolvedType) {
    let index = program
        .functions
        .iter()
        .position(|function| function.id.as_str() == export_id)
        .unwrap_or_else(|| panic!("BASE declares {export_id}"));
    program.functions[index].params[0].ty = ty;
}

// ---------------------------------------------------------------------
// Positive cases
// ---------------------------------------------------------------------

#[test]
fn admits_a_nested_owned_record_with_a_bytes_leaf_and_a_copy_scalar() {
    let program = resolved(BASE);
    let admitted = classify(&program, "classifier.take").unwrap();
    assert_eq!(admitted.export_id(), "classifier.take");
    assert_eq!(admitted.export_name(), "take");
    assert_eq!(
        admitted.input().term,
        "@15:classifier.pair<@15:classifier.leaf<>,i64>"
    );
    assert_eq!(admitted.input().term, admitted.result().term);
    assert_eq!(admitted.input().owned_leaves.len(), 1);
    assert!(admitted
        .record_closure()
        .contains_key("@15:classifier.leaf<>"));
    assert_eq!(admitted.settlement().obligations().len(), 1);
}

#[test]
fn admits_every_grammar_copy_scalar_beside_a_bytes_leaf() {
    let program = resolved(BASE);
    let admitted = classify(&program, "classifier.scalars_take").unwrap();
    assert_eq!(admitted.input().fields.len(), 9);
    assert_eq!(admitted.input().owned_leaves.len(), 1);
}

#[test]
fn admits_different_templates_for_input_and_result() {
    let program = resolved(DIFFERENT_TEMPLATES);
    let admitted = classify(&program, "classifier.split").unwrap();
    assert_ne!(
        admitted.input().template.declaration,
        admitted.result().template.declaration
    );
    assert_eq!(admitted.result().term, "@15:classifier.leaf<>");
}

#[test]
fn admits_the_same_template_with_different_concrete_arguments() {
    let program = resolved(SAME_TEMPLATE_DIFFERENT_ARGUMENTS);
    let admitted = classify(&program, "classifier.recount").unwrap();
    assert_eq!(
        admitted.input().template.declaration,
        admitted.result().template.declaration
    );
    assert_ne!(admitted.input().term, admitted.result().term);
}

#[test]
fn classification_is_deterministic() {
    let program = resolved(BASE);
    let first = classify(&program, "classifier.take").unwrap();
    let second = classify(&program, "classifier.take").unwrap();
    assert_eq!(first.digest(), second.digest());
    assert_eq!(first.settlement().digest(), second.settlement().digest());
}

/// Reparsing identical source independently must reconstruct the same
/// classified subject: nothing is read back from previously emitted output.
#[test]
fn reparsing_the_same_source_reconstructs_the_same_subject() {
    let first = classify(&resolved(BASE), "classifier.take").unwrap();
    let second = classify(&resolved(BASE), "classifier.take").unwrap();
    assert_eq!(first.digest(), second.digest());
}

#[test]
fn display_only_rename_does_not_move_the_digest() {
    let base = classify(&resolved(BASE), "classifier.take").unwrap();
    let renamed = classify(&resolved(RENAMED), "classifier.take").unwrap();
    assert_eq!(base.digest(), renamed.digest());
    assert_eq!(base.export_name(), "take");
    assert_eq!(renamed.export_name(), "accept");
}

const MANY_FIELDS_MODULE: &str = "test.public_generic_classifier_bounds";

/// Build one record named `Big` with exactly `field_count` fields: every
/// field but the last is an admitted Copy scalar (`i64`), and the last is a
/// direct owned `Bytes` leaf, so the fixture always has exactly one owned
/// leaf regardless of `field_count`. `field_count` must be at least 1.
fn many_fields_source(field_count: usize) -> String {
    assert!(field_count >= 1);
    let mut fields = String::new();
    for index in 0..field_count - 1 {
        fields.push_str(&format!(
            "    @id(\"classifier.big.f{index}\")\n    f{index}: i64,\n"
        ));
    }
    fields.push_str("    @id(\"classifier.big.leaf\")\n    leaf: Bytes,\n");
    format!(
        "\nmodule {MANY_FIELDS_MODULE};\n\n\
         @id(\"classifier.big\")\nrecord Big {{\n{fields}}}\n\n\
         @id(\"classifier.big_take\")\nfn big_take(value: own Big) -> Big {{ value }}\n\n\
         @id(\"app.main\")\nfn main() -> i64 {{ 0 }}\n"
    )
}

/// Exact bound: `MAX_FIELDS_PER_RECORD` fields are admitted.
#[test]
fn a_record_at_the_field_count_bound_is_admitted() {
    let source = many_fields_source(MAX_FIELDS_PER_RECORD);
    let admitted = classify(&resolved(&source), "classifier.big_take").unwrap();
    assert_eq!(admitted.input().fields.len(), MAX_FIELDS_PER_RECORD);
    assert_eq!(admitted.input().owned_leaves.len(), 1);
}

/// First-over-bound: one field more than `MAX_FIELDS_PER_RECORD` is refused,
/// and no other module in this repository enforces this bound today (see
/// this module's own documentation).
#[test]
fn a_record_one_field_over_the_bound_is_refused() {
    let source = many_fields_source(MAX_FIELDS_PER_RECORD + 1);
    let error = classify(&resolved(&source), "classifier.big_take").unwrap_err();
    assert_eq!(error, Refusal::BoundExceeded);
    assert_eq!(error.code(), BOUND_EXCEEDED);
}

/// Build a linear chain of `level_count` distinct record declarations,
/// `Level0` through `Level{level_count - 1}`: every level but the last holds
/// one field, `next`, naming the next level; the last level ends the chain
/// with one direct owned `Bytes` leaf instead. `tag` disambiguates every
/// declared identity from any other fixture built by this function in the
/// same test binary.
///
/// `grammar::describe`'s owned-leaf walk (`collect_owned_leaves`) increments
/// its depth counter by exactly one per level of field nesting it descends
/// before ever looking at a leaf, so when this chain is used as the sole
/// `own` parameter type, the leaf sits at exactly depth `level_count` — the
/// same counter [`grammar::MAX_RECORD_DEPTH`] bounds, re-driven here through
/// real compiled source rather than only trusted from the grammar's own
/// tests. Every level is a distinct declaration, so `check_acyclic` never
/// mistakes this chain for a cycle.
fn chain_source(level_count: usize, tag: &str) -> String {
    assert!(level_count >= 1);
    let mut decls = String::new();
    for level in 0..level_count {
        if level + 1 < level_count {
            let next = level + 1;
            decls.push_str(&format!(
                "@id(\"classifier.chain{tag}.level{level}\")\n\
                 record Level{level} {{\n\
                 \x20\x20\x20\x20@id(\"classifier.chain{tag}.level{level}.next\")\n\
                 \x20\x20\x20\x20next: Level{next},\n\
                 }}\n\n"
            ));
        } else {
            decls.push_str(&format!(
                "@id(\"classifier.chain{tag}.level{level}\")\n\
                 record Level{level} {{\n\
                 \x20\x20\x20\x20@id(\"classifier.chain{tag}.level{level}.leaf\")\n\
                 \x20\x20\x20\x20leaf: Bytes,\n\
                 }}\n\n"
            ));
        }
    }
    format!(
        "\nmodule test.public_generic_classifier_chain{tag};\n\n{decls}\
         @id(\"classifier.chain{tag}.take\")\nfn chain_take(value: own Level0) -> Level0 {{ value }}\n\n\
         @id(\"app.main\")\nfn main() -> i64 {{ 0 }}\n"
    )
}

/// Exact bound: a field-nesting chain exactly `MAX_RECORD_DEPTH` levels deep
/// is admitted through real compiled source, not only trusted from the
/// grammar's own budget. Paired with
/// [`nesting_one_level_over_the_depth_bound_is_refused`], the
/// first-over-bound case below — which cannot use this same real-source
/// technique; see that test's own documentation for exactly why.
#[test]
fn a_record_chain_at_the_depth_bound_is_admitted() {
    let source = chain_source(grammar::MAX_RECORD_DEPTH, "_at_depth_bound");
    let admitted =
        classify(&resolved(&source), "classifier.chain_at_depth_bound.take").unwrap();
    assert_eq!(admitted.input().owned_leaves.len(), 1);
}

/// Append `level_count` synthetic record declarations directly to `program`'s
/// already-checked HIR — `Level0` through `Level{level_count - 1}` under
/// `tag`, each a distinct declaration identity, so `check_acyclic` (which
/// tracks active declarations by identity) never mistakes this chain for a
/// self-reference — and return `Level0`'s type. Every level but the last
/// holds one field, `next`, naming the next level; the last ends the chain
/// with one direct owned `Bytes` leaf.
///
/// Built directly onto already-checked HIR rather than through real front-end
/// source, unlike [`chain_source`] above: this fixture is one level deeper
/// than [`grammar::MAX_RECORD_DEPTH`], and
/// `src/source_verify/declaration/declarations.rs`'s `SPX-T268` "owned-Bytes
/// record" front-end profile independently walks every *non-generic* record
/// declaration's own reachable `Bytes` fields against the *same* 64-level
/// depth bound this classifier inherits — so a hand-written `.spx` chain this
/// deep is refused by the front end before `hir::resolve` ever returns a
/// `ResolvedProgram` for this classifier to see at all. This mirrors the
/// technique `crate::hir::type_reachability`'s own tests already use for its
/// analogous depth bound: build the nested type directly in Rust rather than
/// through source that an earlier, unrelated check already forecloses.
fn push_chain_declarations(
    program: &mut ResolvedProgram,
    level_count: usize,
    tag: &str,
) -> ResolvedType {
    assert!(level_count >= 1);
    for level in (0..level_count).rev() {
        let field = if level + 1 < level_count {
            ResolvedFieldDeclaration {
                id: DeclarationId::new(format!("classifier.chain{tag}.level{level}.next")),
                name: "next".to_owned(),
                index: 0,
                ty: nominal(
                    &format!("classifier.chain{tag}.level{}", level + 1),
                    Vec::new(),
                ),
                span: Span::default(),
            }
        } else {
            ResolvedFieldDeclaration {
                id: DeclarationId::new(format!("classifier.chain{tag}.level{level}.leaf")),
                name: "leaf".to_owned(),
                index: 0,
                ty: ResolvedType::Bytes,
                span: Span::default(),
            }
        };
        program.types.push(ResolvedTypeDeclaration {
            id: DeclarationId::new(format!("classifier.chain{tag}.level{level}")),
            name: format!("Level{level}"),
            type_parameters: Vec::new(),
            kind: ResolvedTypeDeclarationKind::Record { fields: vec![field] },
            span: Span::default(),
        });
    }
    nominal(&format!("classifier.chain{tag}.level0"), Vec::new())
}

/// First-over-bound: one field-nesting level deeper than `MAX_RECORD_DEPTH`
/// is refused. Starts from `BASE`'s own already-checked `classifier.take`
/// program and appends the deeper chain as new HIR declarations (see
/// [`push_chain_declarations`] for exactly why this cannot go through real
/// front-end source), then retargets `classifier.take`'s parameter at the
/// chain root. Every level in this chain is a distinct declaration, so this
/// asserts the refusal is `BoundExceeded`, not `RecursiveClosure` — the
/// reason this depth walk could otherwise be confused with, since both are
/// only reachable through a record structure that keeps naming itself.
#[test]
fn nesting_one_level_over_the_depth_bound_is_refused() {
    let mut program = resolved(BASE);
    let root = push_chain_declarations(
        &mut program,
        grammar::MAX_RECORD_DEPTH + 1,
        "_over_depth_bound",
    );
    set_parameter_type(&mut program, "classifier.take", root);

    let error = classify(&program, "classifier.take").unwrap_err();
    assert_eq!(error, Refusal::BoundExceeded);
    assert_eq!(error.code(), BOUND_EXCEEDED);
    assert_ne!(error.code(), RECURSIVE_CLOSURE);
    assert!(!error.diagnostic().message.contains(RECURSIVE_CLOSURE));
}

/// Build the declarations of a perfectly balanced binary tree, `Node0`
/// through `Node{depth}`: `Node0` holds one direct owned `Bytes` leaf, and
/// `Node{k}` (`k >= 1`) holds exactly two fields, `left` and `right`, each of
/// type `Node{k - 1}`. Every declaration this builds has exactly one or two
/// fields (far under `MAX_FIELDS_PER_RECORD`) and the whole tree is only
/// `depth` field-nesting levels deep (far under `MAX_RECORD_DEPTH`), so this
/// fixture reaches a large transitive owned-leaf count purely by width,
/// isolating `MAX_OWNED_LEAVES_PER_INSTANCE` from the other two structural
/// bounds. `Node{depth}` has exactly `2.pow(depth)` transitive owned leaves.
fn balanced_tree_decls(depth: u32, tag: &str) -> String {
    let mut decls = format!(
        "@id(\"classifier.tree{tag}.node0\")\n\
         record Node0 {{\n\
         \x20\x20\x20\x20@id(\"classifier.tree{tag}.node0.leaf\")\n\
         \x20\x20\x20\x20leaf: Bytes,\n\
         }}\n\n"
    );
    for level in 1..=depth {
        let prev = level - 1;
        decls.push_str(&format!(
            "@id(\"classifier.tree{tag}.node{level}\")\n\
             record Node{level} {{\n\
             \x20\x20\x20\x20@id(\"classifier.tree{tag}.node{level}.left\")\n\
             \x20\x20\x20\x20left: Node{prev},\n\
             \x20\x20\x20\x20@id(\"classifier.tree{tag}.node{level}.right\")\n\
             \x20\x20\x20\x20right: Node{prev},\n\
             }}\n\n"
        ));
    }
    decls
}

/// A complete module admitting `Node{depth}` (`2.pow(depth)` owned leaves)
/// directly as the sole `own` parameter.
fn balanced_tree_source(depth: u32, tag: &str) -> String {
    let decls = balanced_tree_decls(depth, tag);
    format!(
        "\nmodule test.public_generic_classifier_tree{tag};\n\n{decls}\
         @id(\"classifier.tree{tag}.take\")\nfn tree_take(value: own Node{depth}) -> Node{depth} {{ value }}\n\n\
         @id(\"app.main\")\nfn main() -> i64 {{ 0 }}\n"
    )
}

/// Exact bound: a balanced tree with exactly `MAX_OWNED_LEAVES_PER_INSTANCE`
/// transitive owned leaves is admitted through real compiled source. The
/// fixture assumes the bound is exactly `2.pow(8)`; if that constant ever
/// changes to a value that is not `2.pow(8)`, this assertion fails loudly
/// instead of silently testing the wrong tree depth. Paired with
/// [`a_record_with_one_leaf_over_the_owned_leaf_bound_is_refused`], the
/// first-over-bound case below — which cannot use this same real-source
/// technique; see that test's own documentation for exactly why.
#[test]
fn a_balanced_tree_at_the_owned_leaf_bound_is_admitted() {
    assert_eq!(
        1usize << 8,
        grammar::MAX_OWNED_LEAVES,
        "update this fixture's tree depth to match the new MAX_OWNED_LEAVES"
    );
    let source = balanced_tree_source(8, "_at_leaf_bound");
    let admitted =
        classify(&resolved(&source), "classifier.tree_at_leaf_bound.take").unwrap();
    assert_eq!(admitted.input().owned_leaves.len(), grammar::MAX_OWNED_LEAVES);
}

/// Append a synthetic balanced binary tree of record declarations, `Node0`
/// through `Node{depth}`, directly to `program`'s already-checked HIR (see
/// [`push_chain_declarations`] for exactly why this cannot go through real
/// front-end source once it is one leaf over the bound), plus one more
/// synthetic `Wrapper` record around `Node{depth}` holding one extra direct
/// `Bytes` field, and return `Wrapper`'s type: exactly `2.pow(depth) + 1`
/// transitive owned leaves, `Wrapper` itself has only two fields (far under
/// the field-count bound), and nesting one record deeper than `Node{depth}`
/// stays far clear of the depth bound too — isolating the leaf-count bound
/// this fixture targets from the other two structural bounds.
fn push_balanced_tree_plus_one_declarations(
    program: &mut ResolvedProgram,
    depth: u32,
    tag: &str,
) -> ResolvedType {
    program.types.push(ResolvedTypeDeclaration {
        id: DeclarationId::new(format!("classifier.tree{tag}.node0")),
        name: "Node0".to_owned(),
        type_parameters: Vec::new(),
        kind: ResolvedTypeDeclarationKind::Record {
            fields: vec![ResolvedFieldDeclaration {
                id: DeclarationId::new(format!("classifier.tree{tag}.node0.leaf")),
                name: "leaf".to_owned(),
                index: 0,
                ty: ResolvedType::Bytes,
                span: Span::default(),
            }],
        },
        span: Span::default(),
    });
    for level in 1..=depth {
        let prev = level - 1;
        let child = nominal(&format!("classifier.tree{tag}.node{prev}"), Vec::new());
        program.types.push(ResolvedTypeDeclaration {
            id: DeclarationId::new(format!("classifier.tree{tag}.node{level}")),
            name: format!("Node{level}"),
            type_parameters: Vec::new(),
            kind: ResolvedTypeDeclarationKind::Record {
                fields: vec![
                    ResolvedFieldDeclaration {
                        id: DeclarationId::new(format!("classifier.tree{tag}.node{level}.left")),
                        name: "left".to_owned(),
                        index: 0,
                        ty: child.clone(),
                        span: Span::default(),
                    },
                    ResolvedFieldDeclaration {
                        id: DeclarationId::new(format!("classifier.tree{tag}.node{level}.right")),
                        name: "right".to_owned(),
                        index: 1,
                        ty: child,
                        span: Span::default(),
                    },
                ],
            },
            span: Span::default(),
        });
    }
    program.types.push(ResolvedTypeDeclaration {
        id: DeclarationId::new(format!("classifier.tree{tag}.wrapper")),
        name: "Wrapper".to_owned(),
        type_parameters: Vec::new(),
        kind: ResolvedTypeDeclarationKind::Record {
            fields: vec![
                ResolvedFieldDeclaration {
                    id: DeclarationId::new(format!("classifier.tree{tag}.wrapper.tree")),
                    name: "tree".to_owned(),
                    index: 0,
                    ty: nominal(&format!("classifier.tree{tag}.node{depth}"), Vec::new()),
                    span: Span::default(),
                },
                ResolvedFieldDeclaration {
                    id: DeclarationId::new(format!("classifier.tree{tag}.wrapper.extra")),
                    name: "extra".to_owned(),
                    index: 1,
                    ty: ResolvedType::Bytes,
                    span: Span::default(),
                },
            ],
        },
        span: Span::default(),
    });
    nominal(&format!("classifier.tree{tag}.wrapper"), Vec::new())
}

/// First-over-bound: one more transitive owned leaf than
/// `MAX_OWNED_LEAVES_PER_INSTANCE` is refused as `BoundExceeded`, paired with
/// [`a_balanced_tree_at_the_owned_leaf_bound_is_admitted`] above. Starts from
/// `BASE`'s own already-checked `classifier.take` program; see
/// [`push_balanced_tree_plus_one_declarations`] for why the over-bound case
/// is built directly in HIR rather than through real front-end source.
#[test]
fn a_record_with_one_leaf_over_the_owned_leaf_bound_is_refused() {
    let mut program = resolved(BASE);
    let root = push_balanced_tree_plus_one_declarations(&mut program, 8, "_over_leaf_bound");
    set_parameter_type(&mut program, "classifier.take", root);

    let error = classify(&program, "classifier.take").unwrap_err();
    assert_eq!(error, Refusal::BoundExceeded);
    assert_eq!(error.code(), BOUND_EXCEEDED);
}

// ---------------------------------------------------------------------
// Negative cases: one per closed refusal reason
// ---------------------------------------------------------------------

#[test]
fn unknown_export_is_export_not_found() {
    let program = resolved(BASE);
    let error = classify(&program, "classifier.absent").unwrap_err();
    assert_eq!(error, Refusal::ExportNotFound);
    assert_eq!(error.code(), EXPORT_NOT_FOUND);
}

#[test]
fn a_generic_function_template_is_refused() {
    let program = resolved(BASE);
    let error = classify(&program, "classifier.identity").unwrap_err();
    assert_eq!(error, Refusal::GenericFunctionTemplate);
    assert_eq!(error.code(), GENERIC_FUNCTION_TEMPLATE);
}

#[test]
fn two_parameters_is_wrong_parameter_count() {
    let program = resolved(BASE);
    let error = classify(&program, "classifier.two_params").unwrap_err();
    assert_eq!(error, Refusal::WrongParameterCount { found: 2 });
    assert_eq!(error.code(), WRONG_PARAMETER_COUNT);
}

#[test]
fn a_value_mode_scalar_parameter_is_wrong_ownership_mode() {
    let program = resolved(BASE);
    let error = classify(&program, "classifier.scalar").unwrap_err();
    assert_eq!(error, Refusal::WrongOwnershipMode);
}

#[test]
fn a_borrowed_top_level_parameter_is_wrong_ownership_mode() {
    let program = resolved(BASE);
    let error = classify(&program, "classifier.borrowed").unwrap_err();
    assert_eq!(error, Refusal::WrongOwnershipMode);
    assert_eq!(error.code(), WRONG_OWNERSHIP_MODE);
}

#[test]
fn an_owned_non_record_input_is_unsupported_input_shape() {
    let program = resolved(BASE);
    let error = classify(&program, "classifier.owned_bytes").unwrap_err();
    assert_eq!(error, Refusal::UnsupportedInputShape);
    assert_eq!(error.code(), UNSUPPORTED_INPUT_SHAPE);
}

#[test]
fn a_non_record_result_is_unsupported_result_shape() {
    let program = resolved(BASE);
    let error = classify(&program, "classifier.no_owned_result").unwrap_err();
    assert_eq!(error, Refusal::UnsupportedResultShape);
    assert_eq!(error.code(), UNSUPPORTED_RESULT_SHAPE);
}

#[test]
fn a_declared_effect_is_an_effectful_export() {
    let program = resolved(BASE);
    let error = classify(&program, "classifier.effectful").unwrap_err();
    assert_eq!(error, Refusal::EffectfulExport);
    assert_eq!(error.code(), EFFECTFUL_EXPORT);
}

/// A record whose `own` parameter owns no `Bytes` leaf at all cannot be
/// reached through real front-end-checked source: the resolver's own
/// `SPX-O002` ("ownership mode `own` is only valid for resource types")
/// already refuses declaring `own` on a value-only record before this
/// classifier ever sees it. To exercise `public_generic_settlement::plan`'s
/// "nothing to settle" refusal anyway, mutate a real compiled program's
/// `Leaf.head` field from `Bytes` to a Copy scalar after checking — the same
/// adversarial-mutation technique this module uses for the other reasons a
/// well-formed front end already forecloses.
#[test]
fn an_own_parameter_with_no_owned_leaf_anywhere_is_a_cleanup_inventory_mismatch() {
    let mut program = resolved(BASE);
    set_leaf_head_type(&mut program, ResolvedType::I64);
    let error = classify(&program, "classifier.take").unwrap_err();
    assert_eq!(error, Refusal::CleanupInventoryMismatch);
    assert_eq!(error.code(), CLEANUP_INVENTORY_MISMATCH);
}

#[test]
fn two_declarations_sharing_one_identity_is_ambiguous_stable_identity() {
    let mut program = resolved(BASE);
    let scalar_index = program
        .functions
        .iter()
        .position(|function| function.id.as_str() == "classifier.scalar")
        .unwrap();
    let mut duplicate = program.functions[scalar_index].clone();
    let take_id = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "classifier.take")
        .unwrap()
        .id
        .clone();
    duplicate.id = take_id;
    program.functions.push(duplicate);

    let error = classify(&program, "classifier.take").unwrap_err();
    assert_eq!(error, Refusal::AmbiguousStableIdentity);
    assert_eq!(error.code(), AMBIGUOUS_STABLE_IDENTITY);
}

/// Arity-mismatch sub-case: one *missing* type argument (one supplied where
/// `Pair<T, U>` declares two). Paired with
/// [`an_argument_count_above_declared_arity_is_arity_mismatch`] (one *extra*
/// argument) and
/// [`swapping_two_type_arguments_of_the_same_arity_is_not_an_arity_mismatch`]
/// (same count, reordered): all three collapse into this one classifier
/// reason and code, but each is driven by a genuinely different input shape
/// rather than only the first ever being exercised.
#[test]
fn an_argument_count_below_declared_arity_is_arity_mismatch() {
    let mut program = resolved(BASE);
    set_parameter_type(
        &mut program,
        "classifier.take",
        nominal("classifier.pair", vec![ResolvedType::Bytes]),
    );
    let error = classify(&program, "classifier.take").unwrap_err();
    assert_eq!(error, Refusal::ArityMismatch);
    assert_eq!(error.code(), ARITY_MISMATCH);
}

/// Arity-mismatch sub-case: one *extra* (duplicated) type argument beyond
/// `Pair<T, U>`'s declared arity of two. Same reason and code as the
/// missing-argument case above, driven by the opposite direction of count
/// mismatch, so a classifier that only ever checked "too few" could not pass
/// this test.
#[test]
fn an_argument_count_above_declared_arity_is_arity_mismatch() {
    let mut program = resolved(BASE);
    set_parameter_type(
        &mut program,
        "classifier.take",
        nominal(
            "classifier.pair",
            vec![
                nominal("classifier.leaf", Vec::new()),
                ResolvedType::I64,
                ResolvedType::I64,
            ],
        ),
    );
    let error = classify(&program, "classifier.take").unwrap_err();
    assert_eq!(error, Refusal::ArityMismatch);
    assert_eq!(error.code(), ARITY_MISMATCH);
}

/// Arity-mismatch sub-case that is *not* a refusal at all: two type
/// arguments at the correct declared count, reordered. `classify_with`'s
/// arity check is a bare length comparison
/// (`found.type_parameters.len() != arguments.len()`), so a same-count
/// reorder never trips `ArityMismatch`; it is admitted as a legitimately
/// different instance. This is the paired legal-input counterpart to the two
/// tests above: it proves the classifier distinguishes "wrong count" from
/// "same count, different order" rather than conflating every argument-list
/// change into one reason.
///
/// A real, separately compiled program is used here rather than mutating
/// `BASE`'s checked `classifier.take`: swapping which type argument lands in
/// `Pair`'s `left` vs. `right` field moves which field carries the owned
/// `Bytes` leaf, and an in-place HIR mutation would leave the compiler's own
/// cleanup inventory pointing at the old (pre-swap) leaf path, tripping
/// `CleanupInventoryMismatch` instead of proving the swap is legal. Compiling
/// `SWAPPED_ARGUMENTS` from scratch gives the swapped function its own
/// internally consistent cleanup facts, as a real front end would.
#[test]
fn swapping_two_type_arguments_of_the_same_arity_is_not_an_arity_mismatch() {
    let original = classify(&resolved(BASE), "classifier.take").unwrap();
    let swapped = classify(&resolved(SWAPPED_ARGUMENTS), "classifier.swap_take").unwrap();

    assert_ne!(swapped.input().term, original.input().term);
    assert_eq!(swapped.input().arguments.len(), 2);
    assert_eq!(swapped.input().owned_leaves.len(), 1);
}

#[test]
fn an_unsubstituted_type_parameter_reaching_the_closure_is_unresolved_type_argument() {
    let mut program = resolved(BASE);
    set_parameter_type(
        &mut program,
        "classifier.take",
        nominal(
            "classifier.pair",
            vec![
                ResolvedType::TypeParameter {
                    owner: DeclarationId::new("classifier.pair"),
                    index: 0,
                },
                ResolvedType::I64,
            ],
        ),
    );
    let error = classify(&program, "classifier.take").unwrap_err();
    assert_eq!(error, Refusal::UnresolvedTypeArgument);
    assert_eq!(error.code(), UNRESOLVED_TYPE_ARGUMENT);
}

/// A compiler-owned nominal reached as an instance *argument* (a top-level
/// generic parameter position, validated by `grammar::classify_with`, which
/// checks `is_compiler_owned_id` before ever looking the declaration up) is
/// `TypeOutsideGrammar`. Contrast this with a compiler-owned nominal reached
/// as a nested *field* two or more levels deep, which `grammar`'s owned-leaf
/// walk validates only by the found declaration's own kind rather than by
/// `is_compiler_owned_id` — since `Option`/`Result` are themselves
/// implemented as authored variants internally, that path instead produces
/// `VariantResourceOrFunctionValue`. Both refuse; this test pins the
/// argument-position case specifically, and the nested-field case is
/// covered by [`a_variant_field_is_variant_resource_or_function_value`]
/// using the same underlying `core.option`-shaped declaration kind.
#[test]
fn a_compiler_owned_nominal_argument_is_type_outside_grammar() {
    let mut program = resolved(BASE);
    set_parameter_type(
        &mut program,
        "classifier.take",
        nominal(
            "classifier.pair",
            vec![
                nominal(crate::prelude::OPTION_ID, vec![ResolvedType::I64]),
                ResolvedType::I64,
            ],
        ),
    );
    let error = classify(&program, "classifier.take").unwrap_err();
    assert_eq!(error, Refusal::TypeOutsideGrammar);
    assert_eq!(error.code(), TYPE_OUTSIDE_GRAMMAR);
}

#[test]
fn a_borrowed_text_view_field_is_borrowed_field_present() {
    let mut program = resolved(BASE);
    set_leaf_head_type(&mut program, ResolvedType::Str);
    let error = classify(&program, "classifier.take").unwrap_err();
    assert_eq!(error, Refusal::BorrowedFieldPresent);
    assert_eq!(error.code(), BORROWED_FIELD_PRESENT);
}

#[test]
fn a_borrowed_byte_view_field_is_borrowed_field_present() {
    let mut program = resolved(BASE);
    set_leaf_head_type(&mut program, ResolvedType::SliceU8);
    let error = classify(&program, "classifier.take").unwrap_err();
    assert_eq!(error, Refusal::BorrowedFieldPresent);
}

#[test]
fn a_variant_field_is_variant_resource_or_function_value() {
    let mut program = resolved(BASE);
    set_leaf_head_type(&mut program, nominal("classifier.status", Vec::new()));
    let error = classify(&program, "classifier.take").unwrap_err();
    assert_eq!(error, Refusal::VariantResourceOrFunctionValue);
    assert_eq!(error.code(), VARIANT_RESOURCE_OR_FUNCTION_VALUE);
}

#[test]
fn a_self_referential_record_field_is_a_recursive_closure() {
    let mut program = resolved(BASE);
    set_leaf_head_type(&mut program, nominal("classifier.leaf", Vec::new()));
    let error = classify(&program, "classifier.take").unwrap_err();
    assert_eq!(error, Refusal::RecursiveClosure);
    assert_eq!(error.code(), RECURSIVE_CLOSURE);
}

#[test]
fn a_field_naming_a_declaration_absent_from_the_program_is_incompatible_retained_facts() {
    let mut program = resolved(BASE);
    set_leaf_head_type(
        &mut program,
        nominal("classifier.does_not_exist", Vec::new()),
    );
    let error = classify(&program, "classifier.take").unwrap_err();
    assert_eq!(error, Refusal::IncompatibleRetainedFacts);
    assert_eq!(error.code(), INCOMPATIBLE_RETAINED_FACTS);
}

/// `SettlementObligationMismatch` is a real, allocated diagnostic in the
/// closed vocabulary, but no fixture in this module produces it as a
/// condition distinct from `CleanupInventoryMismatch` — see this module's
/// own "Known limitations" documentation for exactly why. This test pins
/// only that the reason and its code exist and render, so the vocabulary
/// stays exercised even though the reason itself is currently unreachable
/// through this classifier's own logic.
#[test]
fn settlement_obligation_mismatch_reason_and_code_are_defined() {
    let refusal = Refusal::SettlementObligationMismatch;
    assert_eq!(refusal.code(), SETTLEMENT_OBLIGATION_MISMATCH);
    assert_eq!(refusal.reason(), "settlement_obligation_mismatch");
    assert_eq!(refusal.diagnostic().code, SETTLEMENT_OBLIGATION_MISMATCH);
}

// ---------------------------------------------------------------------
// Separation
// ---------------------------------------------------------------------

/// This classifier is additive: it must not change how any existing
/// projection over the identical checked facts already used above decides
/// admission. `CandidateSurface` (PG-3) and `public_generic_settlement`
/// (PG-7) are the two projections this classifier composes; re-deriving
/// their verdicts independently over the same program proves this module
/// adds a predicate rather than silently replacing one.
#[test]
fn admission_agrees_with_the_surface_and_settlement_projections_it_composes() {
    let program = resolved(BASE);
    let admitted = classify(&program, "classifier.take").unwrap();

    let surface = CandidateSurface::derive(&program, &["classifier.take".to_owned()]).unwrap();
    assert_eq!(
        surface.entries()["classifier.take"].parameters[0]
            .value
            .term,
        admitted.input().term
    );

    let inventory = TypeInventory::of(&program);
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "classifier.take")
        .unwrap();
    let plan = settlement::plan(&inventory, function, 0).unwrap();
    assert_eq!(plan.digest(), admitted.settlement().digest());
}
