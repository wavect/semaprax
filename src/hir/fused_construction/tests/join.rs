//! Exact source/HIR join definitions over the actual authenticated source
//! `SourceProof`. Sources are parsed, not checked: SPX-T267 still refuses every
//! selected count, and a joined value is received proof data, not admission.
//! Positive joins receive the selected factory's HIR for the exact source;
//! hostile controls use the `rescaled` stand-in: mutated, not a checked program.
use super::super::join::{current_source, join};
use super::*;

type Forgery = (&'static str, fn(&mut ResolvedProgram));
use crate::ast;
use crate::hir::{FunctionExecutionId, ValueId};

fn parsed(text: &str) -> ast::Program {
    crate::parse(text, "fused-construction-join.spx").unwrap()
}

fn joins(source: &ast::Program, program: &ResolvedProgram) -> Result<(), Refusal> {
    let proof = current_source(source)?;
    join(source, proof, program).map(|_| ())
}

fn tail(program: &mut ResolvedProgram) -> &mut E {
    let K::Block { tail, .. } = &mut program.functions[0].body.kind else {
        panic!("body")
    };
    tail
}

/// Generated statements of the lowered borrowed-traversal region.
fn region(program: &mut ResolvedProgram) -> &mut Vec<S> {
    let S::Let { value, .. } = block(program)
        .iter_mut()
        .find(|s| matches!(s, S::Let { binding, .. } if binding.name == "#for-borrow"))
        .expect("traversal")
    else {
        panic!("traversal")
    };
    let K::Block { statements, .. } = &mut value.kind else {
        panic!("region")
    };
    statements
}

fn length_call(program: &mut ResolvedProgram) -> &mut E {
    let S::Let { value, .. } = &mut region(program)[0] else {
        panic!("length")
    };
    value
}

fn generated_counter(program: &mut ResolvedProgram) -> ValueId {
    let S::Let { binding, .. } = &region(program)[1] else {
        panic!("counter")
    };
    binding.id.clone()
}

/// Rename the generated counter binding and every place naming it.
fn rename_counter(program: &mut ResolvedProgram, id: ValueId) {
    fn rename(expression: &mut E, from: &ValueId, to: &ValueId) {
        match &mut expression.kind {
            K::Place(place) if place.root == *from => place.root = to.clone(),
            K::Binary { left, right, .. } => {
                rename(left, from, to);
                rename(right, from, to);
            }
            _ => {}
        }
    }
    let from = generated_counter(program);
    let statements = region(program);
    let S::Let { binding, .. } = &mut statements[1] else {
        panic!("counter")
    };
    binding.id = id.clone();
    let (S::While {
        condition, body, ..
    }
    | S::LexicalWhile {
        condition, body, ..
    }) = &mut statements[2]
    else {
        panic!("iteration")
    };
    rename(condition, &from, &id);
    let K::Block { statements, .. } = &mut body.kind else {
        panic!("iteration body")
    };
    let S::Let {
        value: snapshot, ..
    } = &mut statements[0]
    else {
        panic!("snapshot")
    };
    rename(snapshot, &from, &id);
    let S::Assign { binding, value, .. } = &mut statements[2] else {
        panic!("increment")
    };
    binding.id = id.clone();
    rename(value, &from, &id);
}

/// Statements of the authored traversal body inside its lowered region.
fn authored(program: &mut ResolvedProgram) -> &mut Vec<S> {
    let statements = region(program);
    let (S::While { body, .. } | S::LexicalWhile { body, .. }) = &mut statements[2] else {
        panic!("iteration")
    };
    let K::Block { statements, .. } = &mut body.kind else {
        panic!("iteration body")
    };
    let S::Let { value, .. } = &mut statements[1] else {
        panic!("authored")
    };
    let K::Block { statements, .. } = &mut value.kind else {
        panic!("authored block")
    };
    statements
}

fn sort_operand(program: &mut ResolvedProgram) -> &mut E {
    let S::Let { value, .. } = block(program)
        .iter_mut()
        .find(|s| matches!(s, S::Let { binding, .. } if binding.name == "renewed"))
        .expect("sort")
    else {
        panic!("sort")
    };
    let K::Call { args, .. } = &mut value.kind else {
        panic!("sort call")
    };
    &mut args[0]
}

fn set_root(expression: &mut E, root: ValueId) {
    let K::Place(place) = &mut expression.kind else {
        panic!("place")
    };
    place.root = root;
}

fn main_execution() -> FunctionExecutionId {
    FunctionExecutionId::Monomorphic(DeclarationId::new("app.main"))
}

/// Positive joins use the actual selected factory's HIR resolved from the
/// exact parsed source, so every source span and identity is that source's
/// own. The rescaled 32-row stand-in differs in literal offsets (`  32` vs
/// `4096`) and stays a non-authority fixture for the hostile controls below.
#[test]
fn authenticated_source_proof_joins_the_independent_typed_selection() {
    for count in [33u32, 4096] {
        let text = ORIGINAL.replace("4096", &count.to_string());
        let source = parsed(&text);
        let owner = crate::hir::selected_fused_candidate::validate_selected(&source)
            .unwrap_or_else(|e| panic!("selected factory refused {count}: {e:?}"));
        assert!(std::ptr::eq(owner.source(), &source));
        let program = owner.program();
        let proof = current_source(&source).unwrap();
        assert_eq!(proof.rows(), count);
        assert_eq!(proof.owner_events().count(), 3, "move, borrow and sort");
        let joined = join(&source, proof, program).unwrap();
        assert_eq!(joined.proof().rows(), count);
        let flow = joined.flow();
        assert_eq!((flow.rows, flow.width), (count, 3));
        assert_eq!(flow.producer, joined.selection().producer().id.as_str());
        assert!(joined.selects(flow.condition, flow.body).is_some());
        assert!(joined.selects(flow.body, flow.condition).is_none());
        assert!(joined.still_current(&source, program));
        assert!(!joined.still_current(&source.clone(), program));
        assert!(!joined.still_current(&source, &program.clone()));
        // The offset-shifted rescaled stand-in never joins this exact source.
        assert!(joins(&source, &rescaled(count)).is_err(), "{count}");
        // Closed default: ordinary checking keeps the original refusal.
        assert!(crate::check(&text, "fused-construction-join-refused.spx")
            .unwrap_err()
            .iter()
            .any(|d| d.code == "SPX-T267"));
    }
    let legacy = parsed(&text(8));
    assert_eq!(current_source(&legacy).err(), Some(Refusal::Legacy));
}

#[test]
fn stale_or_foreign_source_proofs_refuse() {
    let source = parsed(&text(64));
    let program = rescaled(64);
    let stale = source.clone();
    let proof = current_source(&stale).unwrap();
    assert_eq!(
        join(&source, proof, &program).err(),
        Some(Refusal::Join(
            "source proof is not authenticated over current source"
        ))
    );
    let other = parsed(&text(64).replace("module app.leaf;", "module app.leap;"));
    assert!(joins(&other, &program).is_err());
    assert!(joins(&source, &program).is_ok());
}

#[test]
fn changed_source_prefix_and_suffix_operands_refuse_against_unchanged_hir() {
    let program = rescaled(64);
    let original = text(64);
    // Each replacement keeps every byte offset, so only the operand differs.
    for (from, to) in [
        ("while next <   64", "while next <   65"),
        ("(  64usize)", "(  65usize)"),
        ("bytes_zeroed(3usize)", "bytes_zeroed(4usize)"),
        ("next = next + 1", "next = next + 2"),
        ("\"é\\u{0}\"", "\"é\\u{1}\""),
        ("while next <   64", "while next <=  64"),
        ("module app.leaf;", "module app.leap;"),
        ("items", "itemz"),
        ("let item = Row", "let iten = Row"),
        (
            "total = total + row.priority",
            "total = total - row.priority",
        ),
        ("count = count + 1", "count = count + 2"),
        ("count ==   64", "count ==   65"),
        ("{ total } else { 0 }", "{ total } else { 1 }"),
        ("value == 195u8", "value == 196u8"),
        ("str_len_bytes(text) == 3", "str_len_bytes(text) == 4"),
        ("let text = row.title", "let texu = row.title"),
    ] {
        assert!(original.contains(from), "{from}");
        let changed = original.replace(from, to);
        assert_eq!(changed.len(), original.len(), "{from}");
        let source = parsed(&changed);
        assert!(joins(&source, &program).is_err(), "{from} -> {to} joined");
    }
    assert!(joins(&parsed(&original), &program).is_ok());
}

#[test]
fn same_span_hir_suffix_and_owner_forgeries_cannot_join_unchanged_source() {
    let source = parsed(&text(64));
    let canonical = crate::format::canonical(&source);
    let forgeries: [Forgery; 13] = [
        ("count", |p| {
            let S::Let { value, .. } = &mut block(p)[0] else {
                panic!("allocation")
            };
            rescale(value, 64, 65);
            rescale(condition(p), 64, 65);
        }),
        ("width", |p| set_width(p, 4)),
        ("text", |p| {
            fields(p)[1].value.kind = K::String("e\u{0}x".to_owned())
        }),
        ("function span", |p| p.functions[0].span.start += 1),
        ("tail literal", |p| rescale(tail(p), 64, 65)),
        ("tail result", |p| {
            if let K::If { else_branch, .. } = &mut tail(p).kind {
                else_branch.kind = K::Int(1);
            }
        }),
        ("suffix operator", |p| {
            let S::Assign { value, .. } = &mut authored(p)[4] else {
                panic!("total")
            };
            if let K::Binary { op, .. } = &mut value.kind {
                *op = BinaryOp::Sub;
            }
        }),
        ("suffix literal", |p| {
            let S::Assign { value, .. } = &mut authored(p)[5] else {
                panic!("count")
            };
            rescale(value, 1, 2);
        }),
        ("view binding", |p| {
            let S::Let { binding, .. } = &mut authored(p)[0] else {
                panic!("text")
            };
            binding.name = "texu".to_owned();
        }),
        ("moved owner sort", |p| {
            let S::Let { binding, .. } = &block(p)[0] else {
                panic!("destination")
            };
            let moved = binding.id.clone();
            set_root(sort_operand(p), moved);
        }),
        ("foreign owner sort", |p| {
            set_root(
                sort_operand(p),
                ValueId::local(&main_execution(), "body.s99"),
            );
        }),
        ("sort target", |p| {
            let S::Let { value, .. } = block(p)
                .iter_mut()
                .find(|s| matches!(s, S::Let { binding, .. } if binding.name == "renewed"))
                .expect("sort")
            else {
                panic!("sort")
            };
            if let K::Call { callee, .. } = &mut value.kind {
                *callee = DeclarationId::new(crate::vec_ops::LEN_ID);
            }
        }),
        ("alias from induction", |p| {
            let S::Let { binding, .. } = &block(p)[1] else {
                panic!("induction")
            };
            let counter = binding.id.clone();
            let S::Let { value, .. } = block(p)
                .iter_mut()
                .find(|s| matches!(s, S::Let { binding, .. } if binding.name == "rows"))
                .expect("alias")
            else {
                panic!("alias")
            };
            set_root(value, counter);
        }),
    ];
    for (name, forge) in forgeries {
        let mut program = rescaled(64);
        forge(&mut program);
        assert_ne!(program, rescaled(64), "{name} changed nothing");
        assert!(
            joins(&source, &program).is_err(),
            "{name} joined unchanged source"
        );
        // The source is unchanged and still joins genuine HIR.
        assert_eq!(crate::format::canonical(&source), canonical);
        assert!(joins(&source, &rescaled(64)).is_ok(), "{name}");
    }
}

#[test]
fn generated_traversal_scaffold_identities_are_canonical() {
    let source = parsed(&text(64));
    // Direct mutants keep the freeze replay's shape and equal roots.
    let direct: [Forgery; 2] = [
        ("foreign length call", |p| {
            length_call(p).id = crate::hir::ExpressionId::new(&main_execution(), "foreign.length");
        }),
        ("renamed generated counter", |p| {
            rename_counter(p, ValueId::local(&main_execution(), "foreign.counter"));
        }),
    ];
    for (name, forge) in direct {
        let mut program = rescaled(64);
        forge(&mut program);
        assert_ne!(program, rescaled(64), "{name} changed nothing");
        // The typed derivation's closed census accepts the replayed shape.
        assert!(derive(&program).is_ok(), "{name}");
        assert_eq!(
            joins(&source, &program).err(),
            Some(Refusal::Join(
                "traversal scaffold identity is not its resolver position"
            )),
            "{name}"
        );
    }
    // Cache-wire mutants: the decoder recomputes every fingerprint.
    let mut original = rescaled(64);
    let counter = generated_counter(&mut original).as_str().to_owned();
    let length = length_call(&mut original).id.as_str().to_owned();
    assert!(counter.ends_with(".value.s1"));
    for (name, from, to) in [
        (
            "wire-renamed generated counter",
            counter.clone(),
            format!("{}q1", &counter[..counter.len() - 2]),
        ),
        (
            "wire-foreign length call",
            length.clone(),
            format!("{}x", &length[..length.len() - 1]),
        ),
    ] {
        let forged = rewire(&original, &from, &to);
        assert!(derive(&forged).is_ok(), "{name}");
        assert_eq!(
            joins(&source, &forged).err(),
            Some(Refusal::Join(
                "traversal scaffold identity is not its resolver position"
            )),
            "{name}"
        );
    }
    assert!(joins(&source, &original).is_ok());
}

#[test]
fn decoded_consistent_hir_renames_cannot_join() {
    let source = parsed(&text(64));
    let original = rescaled(64);
    let selection = derive(&original).unwrap();
    let vector = selection.vector().id.as_str().to_owned();
    let forged = rewire(&original, &vector, &vector.replace("body.s0", "body.q0"));
    assert!(joins(&source, &forged).is_err());
    assert!(joins(&source, &original).is_ok());
}
