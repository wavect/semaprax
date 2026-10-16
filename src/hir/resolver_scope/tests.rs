use super::*;
use crate::hir::*;
use crate::test_allocation_observer::measure;
use std::path::Path;

fn binding(name: &str, ty: ResolvedType, mutable: bool) -> Binding {
    Binding {
        traversal_row: None,
        id: ValueId::new(name.to_owned()),
        ty,
        ownership: OwnershipMode::Value,
        mutable,
    }
}
fn complete_input(
    map: &BTreeMap<String, Binding>,
) -> Vec<(String, ValueId, ResolvedType, OwnershipMode, bool)> {
    map.iter()
        .map(|(name, b)| {
            assert!(b.traversal_row.is_none());
            (
                name.clone(),
                b.id.clone(),
                b.ty.clone(),
                b.ownership,
                b.mutable,
            )
        })
        .collect()
}

#[test]
fn borrowed_root_actual_storage_and_drop_keep_the_entire_input_map() {
    for count in [0, 1, 64, 512] {
        let input: BTreeMap<_, _> = (0..count)
            .map(|i| {
                let name = format!("λ.binding.{i:04}.{}", "wide".repeat(8));
                (
                    name.clone(),
                    binding(
                        &name,
                        ResolvedType::Function {
                            parameters: vec![ResolvedType::I64, ResolvedType::Bool],
                            result: Box::new(ResolvedType::I64),
                        },
                        i % 2 == 0,
                    ),
                )
            })
            .collect();
        let before = complete_input(&input);
        let (_, old) = measure(|| {
            let root = Rc::new(input.clone());
            let children = [root.clone(), root.clone(), root.clone()];
            assert_eq!(root.len(), input.len());
            for (name, id, ty, ownership, mutable) in &before {
                let b = &root[name];
                assert_eq!(
                    (&b.id, &b.ty, &b.ownership, &b.mutable),
                    (id, ty, ownership, mutable)
                );
            }
            drop(children);
            drop(root);
        });
        let (_, current) = measure(|| {
            let root = borrowed(&input);
            let children = [root.clone(), root.clone(), root.clone()];
            assert!(matches!(root.as_ref(), Cow::Borrowed(map) if std::ptr::eq(*map, &input)));
            assert_eq!(root.len(), input.len());
            drop(children);
            drop(root);
        });
        for observation in [old, current] {
            assert!(!observation.invalid);
            assert_eq!(observation.allocated, observation.freed);
            assert_eq!(observation.live, 0);
        }
        assert_eq!(current.allocations, 1);
        assert_eq!(current.reallocations, 0);
        assert_eq!(
            current.allocated,
            std::mem::size_of::<Cow<'_, BTreeMap<String, Binding>>>()
                + 2 * std::mem::size_of::<usize>()
        );
        if count > 0 {
            assert!(current.allocated < old.allocated);
        } else {
            assert_eq!(
                old.allocated,
                std::mem::size_of::<BTreeMap<String, Binding>>() + 2 * std::mem::size_of::<usize>()
            );
            assert!(current.allocated >= old.allocated);
        }
        assert_eq!(complete_input(&input), before);
    }
}

#[test]
fn complete_map_detach_keeps_siblings_every_field_and_real_drop() {
    let input = BTreeMap::from([
        (
            "original".into(),
            binding("original", ResolvedType::I64, true),
        ),
        (
            "signature".into(),
            binding(
                "signature",
                ResolvedType::Function {
                    parameters: vec![ResolvedType::Bool],
                    result: Box::new(ResolvedType::I64),
                },
                false,
            ),
        ),
    ]);
    let before = complete_input(&input);
    let (_, observed) = measure(|| {
        let root = borrowed(&input);
        let mut child = root.clone();
        Rc::make_mut(&mut child)
            .to_mut()
            .insert("local".into(), binding("local", ResolvedType::Bool, false));
        assert!(matches!(root.as_ref(), Cow::Borrowed(_)));
        assert!(matches!(child.as_ref(), Cow::Owned(_)));
        assert!(!root.contains_key("local"));
        assert!(child.contains_key("local"));
        for (name, id, ty, ownership, mutable) in &before {
            let value = &child[name];
            assert_eq!(
                (&value.id, &value.ty, &value.ownership, &value.mutable),
                (id, ty, ownership, mutable)
            );
        }
        let sibling = child.clone();
        Rc::make_mut(&mut child)
            .to_mut()
            .get_mut("original")
            .unwrap()
            .mutable = false;
        assert!(sibling["original"].mutable);
        assert!(!child["original"].mutable);
        assert!(input["original"].mutable);
        assert!(owned_capacity(&root) < owned_capacity(&child));
        drop(sibling);
        drop(child);
        drop(root);
    });
    assert!(!observed.invalid && observed.allocations > 1);
    assert_eq!(observed.allocated, observed.freed);
    assert_eq!(observed.live, 0);
    assert_eq!(complete_input(&input), before);
}

fn receiver(source: &str, require_success: bool) {
    let parsed = crate::parse(source, Path::new("borrowed-root.spx")).unwrap();
    let resolver = crate::hir::Resolver {
        function_cells: std::borrow::Cow::Owned(crate::source_verify::function_cells(&parsed)),
        program: &parsed,
        byte_helpers: crate::loop_calls::byte_writer::SourceIndex::new(&parsed),
        declarations: DeclarationIndex::from_verified(&parsed).unwrap(),
        reuse: None,
        function_work: FunctionResolutionWork::default(),
        fused_producer: None,
        partition_profile: false,
    };
    for function in &parsed.functions {
        let execution =
            FunctionExecutionId::Monomorphic(DeclarationId::new(function.stable_id.clone()));
        let input: BTreeMap<_, _> = function
            .params
            .iter()
            .enumerate()
            .map(|(index, param)| {
                let ty = resolver
                    .resolve_function_type(function, &param.ty, param.span)
                    .unwrap();
                (
                    param.name.clone(),
                    Binding {
                        traversal_row: None,
                        id: ValueId::parameter(&execution, index),
                        ty,
                        ownership: param.mode.into(),
                        mutable: false,
                    },
                )
            })
            .collect();
        let before = complete_input(&input);
        for root in ["body", "contract.requires.0", "λ.根", ""] {
            let expected = crate::hir::borrowed_root_oracle::Resolver(&resolver).expression(
                &execution,
                &function.body,
                &input,
                root,
            );
            let current = resolver.resolve_expr_iterative(&execution, &function.body, &input, root);
            // Eq compares every HIR and Diagnostic field; never only a code/message subset.
            assert_eq!(current, expected, "{} at {root}", function.name);
            if require_success {
                assert!(current.is_ok(), "{current:?}");
            } else {
                assert!(
                    current.is_err(),
                    "refusal fixture unexpectedly admitted: {source}"
                );
            }
            // Original and borrowed-root receivers own their complete result and
            // selected diagnostic through actual Drop inside the same observer.
            for old in [false, true] {
                let (_, observed) = measure(|| {
                    let received = if old {
                        crate::hir::borrowed_root_oracle::Resolver(&resolver).expression(
                            &execution,
                            &function.body,
                            &input,
                            root,
                        )
                    } else {
                        resolver.resolve_expr_iterative(&execution, &function.body, &input, root)
                    };
                    assert_eq!(received, expected);
                    drop(received);
                });
                assert!(!observed.invalid);
                assert_eq!(observed.allocated, observed.freed);
                assert_eq!(observed.live, 0);
            }
            assert_eq!(complete_input(&input), before);
        }
    }
}

#[test]
fn complete_frozen_iterative_receiver_preserves_scopes_order_paths_and_ownership() {
    receiver(
        r#"module borrowed.root;
@id("pair") record Pair { @id("pair.a") a:i64, @id("pair.b") b:i64, }
@id("choice") variant Choice { @id("choice.a") A { @id("choice.a.v") v:i64, }, @id("choice.b") B, }
@id("add") fn add(a:i64,b:i64)->i64 {a+b}
@id("value") fn value(flag:bool, choice:Choice, pair:Pair)->i64 {
 let first=add(1,2); let mut total=first;
 if flag { let inner=3; total=total+inner; 0 } else { let inner=4; total=total+inner; 0 }
 while total<0 {total=total+1; if total==2 {continue;} else {0} if total==3 {break;} else {0} 0}
 let rebuilt=pair with {b:total};
 let matched=match rebuilt {Pair {a,b}=>a+b,};
 let selected=match choice {Choice::A{v}=>v,Choice::B{}=>matched,};
 let scalar=match total {0=>selected,rest=>rest,};
 let values=vec_push<i64>(vec_with_capacity<i64>(2usize),scalar);
 for item in values {total=total+item;0}
 let strings=vec_push<string>(vec_with_capacity<string>(1usize),"owned");
 for own text in vec_into_iter<string>(strings) {let kept=text;0}
 if flag && false {return total;} else {0}
 total
}
@id("main") fn main()->i64 {0}
"#,
        true,
    );
}

#[test]
fn frozen_closure_receiver_keeps_owned_capture_publication_and_borrow_boundaries() {
    receiver(
        r#"module borrowed.capture;
@id("copy") fn copy(value:i64)->i64 {let call=fn(x:i64)->i64 {value+x}; call(2)}
@id("main") fn main()->i64 {copy(7)}
"#,
        true,
    );
}

#[test]
fn complete_frozen_iterative_receiver_keeps_every_diagnostic_field_and_first_error() {
    for body in [
        "missing",
        "let value=missing; later",
        "let value=1; value=2; value",
        "if missing {later} else {another}",
        "if true {let inner=2; inner} else {inner}",
        "1+missing",
        "break; 0",
        "continue; 0",
        "let values=vec_with_capacity<i64>(1usize); for values in values {0} 0",
    ] {
        let source = format!("module borrowed.refusal; @id(\"main\") fn main()->i64 {{{body}}}");
        if matches!(body, "break; 0" | "continue; 0") {
            let diagnostic = crate::parse(&source, Path::new("borrowed-root.spx")).unwrap_err();
            assert_eq!(diagnostic.code, "SPX-P209");
            assert_eq!(
                diagnostic.message,
                "code after an unconditional return is unreachable"
            );
            let admitted = source.replace(body, body.strip_suffix(" 0").unwrap());
            receiver(&admitted, false);
        } else {
            receiver(&source, false);
        }
    }
}

#[test]
fn complete_frozen_iterative_receiver_keeps_mixed_binary_lowering_and_source_refusal() {
    // Preserve the original hostile source. Expression lowering records both
    // operand types; source verification owns the mismatch diagnostic, before
    // either full resolver can publish checked HIR.
    let source = "module borrowed.refusal; @id(\"main\") fn main()->i64 {1+true}";
    receiver(source, true);
    let path = Path::new("borrowed-root.spx");
    let diagnostics = crate::check(source, path).unwrap_err();
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>(),
        ["SPX-T208"]
    );
    let parsed = crate::parse(source, path).unwrap();
    // Complete vector equality retains every diagnostic field and its order.
    assert_eq!(crate::hir::resolve(&parsed).unwrap_err(), diagnostics);
}

#[test]
fn detached_traversal_rows_keep_complete_binding_metadata_and_actual_drop() {
    use crate::hir::resolve_for::record::Row;
    let execution = FunctionExecutionId::Monomorphic(DeclarationId::new("row.owner"));
    let source = ResolvedBinding {
        id: ValueId::new("row.source".to_owned()),
        name: "rows".into(),
        ty: crate::vec_ops::resolved_vec(ResolvedType::I64),
        ownership: OwnershipMode::Borrow,
        span: crate::ast::Span::default(),
    };
    let index = ResolvedBinding {
        id: ValueId::new("row.index".to_owned()),
        name: "at".into(),
        ty: ResolvedType::Usize,
        ownership: OwnershipMode::Value,
        span: crate::ast::Span::default(),
    };
    let region = ExpressionId::new(&execution, "body.s0.loop");
    let bound = ValueId::new("row.bound".to_owned());
    let input = BTreeMap::from([(
        "row".into(),
        Binding {
            traversal_row: Some(Box::new(Row {
                source: source.clone(),
                index: index.clone(),
                element: ResolvedType::I64,
                region: region.clone(),
                bound: bound.clone(),
            })),
            id: index.id.clone(),
            ty: ResolvedType::I64,
            ownership: OwnershipMode::Borrow,
            mutable: false,
        },
    )]);
    let (_, observed) = measure(|| {
        let root = borrowed(&input);
        let mut child = root.clone();
        Rc::make_mut(&mut child)
            .to_mut()
            .insert("extra".into(), binding("extra", ResolvedType::Bool, false));
        let original = input["row"].traversal_row.as_deref().unwrap();
        let detached = child["row"].traversal_row.as_deref().unwrap();
        assert!(!std::ptr::eq(original, detached));
        assert_eq!(
            (
                &detached.source,
                &detached.index,
                &detached.element,
                &detached.region,
                &detached.bound
            ),
            (&source, &index, &ResolvedType::I64, &region, &bound)
        );
        assert_eq!(
            (
                &child["row"].id,
                &child["row"].ty,
                child["row"].ownership,
                child["row"].mutable
            ),
            (&index.id, &ResolvedType::I64, OwnershipMode::Borrow, false)
        );
        assert!(std::ptr::eq(
            root["row"].traversal_row.as_deref().unwrap(),
            original
        ));
        drop(child);
        drop(root);
    });
    assert!(!observed.invalid && observed.allocations > 1);
    assert_eq!(observed.allocated, observed.freed);
    assert_eq!(observed.live, 0);
    assert_eq!(input.len(), 1);
}

const GENERIC_TEMPLATE_CLOSURE: &str = r#"
module test.generic_template_closure;
@id("gc.fill") fn fill<T>(values:own Vec<T>,replacement:T)->Vec<T>{
    let length=vec_len<T>(values);
    let mut output=vec_with_capacity<T>(length);
    let mut index=0usize;
    while index<length {
        let callback=fn(item:T)->T{replacement};
        output=vec_push<T>(output,callback(vec_get<T>(values,index)));
        index=index+1usize;
        index<length
    }
    output
}
@id("gc.main") fn main()->i64{
    let i64s=fill<i64>(vec_push<i64>(vec_with_capacity<i64>(1usize),1),2);
    let i32s=fill<i32>(vec_push<i32>(vec_with_capacity<i32>(1usize),1i32),2i32);
    let u8s=fill<u8>(vec_push<u8>(vec_with_capacity<u8>(1usize),1u8),2u8);
    let usizes=fill<usize>(vec_push<usize>(vec_with_capacity<usize>(1usize),1usize),2usize);
    let chars=fill<char>(vec_push<char>(vec_with_capacity<char>(1usize),'a'),'b');
    let f32s=fill<f32>(vec_push<f32>(vec_with_capacity<f32>(1usize),1.0f32),2.0f32);
    let f64s=fill<f64>(vec_push<f64>(vec_with_capacity<f64>(1usize),1.0f64),2.0f64);
    let bools=fill<bool>(vec_push<bool>(vec_with_capacity<bool>(1usize),true),false);
    if vec_len<i64>(i64s)+vec_len<i32>(i32s)+vec_len<u8>(u8s)+vec_len<usize>(usizes)+vec_len<char>(chars)+vec_len<f32>(f32s)+vec_len<f64>(f64s)+vec_len<bool>(bools)==8usize {0} else {1}
}
"#;

#[test]
fn frozen_generic_closure_receiver_keeps_held_owner_projection_and_actual_drop() {
    // Symbolic T captures reach the migrated owner query; concrete scalar
    // captures alone would short-circuit before it. The full original receiver
    // compares all HIR/Diagnostic fields and observes both results through Drop.
    receiver(GENERIC_TEMPLATE_CLOSURE, true);
}
