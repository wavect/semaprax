//! Resolver sites that reconstruct an identity already held by the same pass's
//! `DeclarationIndex`. The saved policy below is the complete old behavior at
//! those thirteen sites (source `String` clone plus a new shared backing); the
//! resolution algorithm itself is the one shared implementation. Agent, role,
//! operation, and helper identities are not index-held and stay unchanged.
use std::cell::{Cell, RefCell};

use super::*;
use crate::ast::{Program, Span, Type, TypeDeclarationKind};
use crate::hir::ids::FunctionExecutionId;
use crate::hir::{
    FunctionResolutionWork, ResolvedProgram, ResolvedResourceDropKind, ResolvedType,
    ResolvedTypeDeclarationKind, Resolver,
};

thread_local! {
    static SAVED: Cell<bool> = const { Cell::new(false) };
    static REQUESTS: Cell<usize> = const { Cell::new(0) };
    static TEXTS: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

/// Called by `DeclarationIndex::held_identity` in test builds only. `None`
/// selects the actual shared-carrier lookup.
pub(in crate::hir::declaration_index) fn saved_identity(text: &str) -> Option<DeclarationId> {
    if !SAVED.with(Cell::get) {
        return None;
    }
    REQUESTS.with(|requests| requests.set(requests.get() + 1));
    TEXTS.with(|texts| {
        if let Some(texts) = texts.borrow_mut().as_mut() {
            texts.push(text.to_owned());
        }
    });
    Some(DeclarationId::new(text.to_owned()))
}

/// Runs `run` under the saved construction policy. Recording allocates, so
/// allocation measurements use `record == false`; the census run records.
fn saved<T>(record: bool, run: impl FnOnce() -> T) -> (T, Vec<String>) {
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            SAVED.with(|saved| saved.set(false));
        }
    }
    REQUESTS.with(|requests| requests.set(0));
    TEXTS.with(|texts| *texts.borrow_mut() = record.then(Vec::new));
    SAVED.with(|saved| saved.set(true));
    let restore = Restore;
    let value = run();
    drop(restore);
    let texts = TEXTS
        .with(|texts| texts.borrow_mut().take())
        .unwrap_or_default();
    (value, texts)
}

fn requests() -> usize {
    REQUESTS.with(Cell::get)
}

/// `(saved old policy, actual shared policy)` over the same unchanged algorithm.
fn pair<T>(run: impl Fn() -> T) -> (T, T) {
    let (old, _) = saved(false, &run);
    (old, run())
}

/// Independent `Arc<str>` layout: two atomic counters plus text, rounded to
/// machine-word alignment. The old constructor also cloned the source `String`.
fn backing(length: usize) -> usize {
    let word = std::mem::size_of::<usize>();
    (2 * word + length).div_ceil(word) * word
}

fn removed_bytes(texts: &[String]) -> usize {
    texts
        .iter()
        .map(|text| text.len() + backing(text.len()))
        .sum()
}

const CENSUS: &str = r#"module resolver.held;
@id("token.type") resource Token { @id("token.drop") drop import "host.dispose"; }
@id("latch.type") resource Latch { @id("latch.drop") drop trivial; }
@id("row.type") record Row { @id("row.x") x:i64, }
@id("base.type") class Base {
    @id("base.x") x:i64,
    @id("base.read") fn read(self:Base)->i64 {self.x}
}
@id("child.type") class Child : Base {
    @id("child.y") y:i64,
    @id("child.read") fn read(self:Child)->i64 {self.y}
}
@id("choice.type") variant Choice {
    @id("choice.left") Left { @id("choice.left.x") x:i64, },
    @id("choice.right") Right,
}
@id("host.interface") interface Host permits {} {
    @id("host.dispose") import fn dispose(token: own Token) -> unit
        effects {}
        failure infallible
        consumes token always;
}
@id("app.discard") fn discard(token: own Token) -> i64 { 0 }
@id("app.main") fn main()->i64 {0}
"#;

const GENERIC: &str = r#"module resolver.held.generic;
@id("counter") class Counter {
 @id("counter.value") value:i64,
 @id("counter.get") fn get(self:Counter)->i64 {self.value}
 @id("counter.bump") fn bump(self:Counter)->Counter {Counter{value:self.value+1}}
}
@id("identity") fn identity<T>(value:T)->T {value}
@id("zed") fn zed()->i64 {1}
@id("alpha") fn alpha()->i64 {zed()}
@id("app.main") fn main()->i64 {identity<i64>(alpha())}
"#;

const INVARIANT: &str = r#"module resolver.held.invariant;
@id("gate.type") record Gate { @id("gate.n") n:i64, } requires n>0
@id("app.main") fn main()->i64 { Gate { n: 1 }.n }
"#;

/// Real admitted callable signatures: `fn(T)->T` / `fn(T,T)->T` parameters on
/// generic templates, instantiated through actual function values. Syntax is
/// the owning `tests/owned_data/generic_owned_function_runtime` fixture.
const CALLABLE: &str = r#"module test.function_values_collections;
@id("adapter.map") fn map<T>(values:own Vec<T>, callback:fn(T)->T)->Vec<T> {
 let count=vec_len<T>(values);
 let mut output=vec_with_capacity<T>(count);
 let mut index=0usize;
 while index<count {
  output=vec_push<T>(output,callback(vec_get<T>(values,index)));
  index=index+1usize;
  true
 }
 output
}
@id("adapter.filter") fn filter<T>(values:own Vec<T>, predicate:fn(T)->bool)->Vec<T> {
 let count=vec_len<T>(values);
 let mut output=vec_with_capacity<T>(count);
 let mut index=0usize;
 while index<count {
  let item=vec_get<T>(values,index);
  let kept=if predicate(item) {output=vec_push<T>(output,item);true}else{false};
  index=index+1usize;
  kept
 }
 output
}
@id("adapter.fold") fn fold<T>(values:own Vec<T>, initial:T, combine:fn(T,T)->T)->T {
 let count=vec_len<T>(values);
 let mut accumulator=initial;
 let mut index=0usize;
 while index<count {
  accumulator=combine(accumulator,vec_get<T>(values,index));
  index=index+1usize;
  true
 }
 accumulator
}
@id("adapter.increment") fn increment(value:i64)->i64 {value+1}
@id("adapter.even") fn even(value:i64)->bool {value%2==0}
@id("adapter.append") fn append(prefix:i64,value:i64)->i64 {prefix*10+value}
@id("app.main") fn main()->i64 {
 let first=vec_push<i64>(vec_with_capacity<i64>(3usize),1);
 let second=vec_push<i64>(first,2);
 let input=vec_push<i64>(second,3);
 let mapped=map<i64>(input,increment);
 let selected=filter<i64>(mapped,even);
 let observed=fold<i64>(selected,0,append);
 let empty=filter<i64>(vec_with_capacity<i64>(0usize),even);
 let unchanged=fold<i64>(empty,7,append);
 if observed==24 && unchanged==7 {8}else{0}
}
"#;

/// Captured and shadowed anonymous closures; the owning fixture is
/// `canonical_loan_storage/tests.rs`. Monomorphic: it reaches the function
/// owner sites, not the generic repeated-owner sites.
const CLOSURES: &str = r#"module loan.closure_storage;
@id("snapshot") fn snapshot()->i64 {
 let mut captured=7;
 let callback=fn(value:i64)->i64 {captured+value};
 captured=100;
 callback(5)
}
@id("shadow") fn shadow(captured:i64)->i64 {
 let callback=fn(captured:i64)->i64 {captured+1};
 callback(41)
}
@id("main") fn main()->i64 {snapshot()+shadow(99)}
"#;

/// Generic closure capturing a template parameter value; the owning fixture
/// is `closure/tests.rs`. The closure signature and body reach the repeated
/// generic owner sites through the closure resolver's receivers.
const GENERIC_CLOSURE: &str = r#"module test.generic_closures;
@id("gc.fill") fn fill<T>(values:own Vec<T>, replacement:T)->Vec<T>{
 let callback=fn(item:T)->T{let mut copy:T=item;copy=replacement;copy};
 vec_push<T>(values,callback(replacement))
}
@id("gc.main") fn main()->i64{
 let left=fill<i64>(vec_with_capacity<i64>(1usize),7);
 let right=fill<bool>(vec_with_capacity<bool>(1usize),true);
 vec_get<i64>(left,0usize)
}"#;

/// Every genuine admitted positive fixture: `(source, path, has invariants)`.
const FIXTURES: [(&str, &str, bool); 7] = [
    (CENSUS, "resolver-held-census.spx", false),
    (GENERIC, "resolver-held-generic.spx", false),
    (INVARIANT, "resolver-held-invariant.spx", true),
    (super::SOURCE, "resolver-held-classes.spx", false),
    (CALLABLE, "resolver-held-callable.spx", false),
    (CLOSURES, "resolver-held-closures.spx", false),
    (GENERIC_CLOSURE, "resolver-held-generic-closure.spx", false),
];

fn named(name: &str, arguments: Vec<Type>) -> Type {
    Type::Named {
        name: name.to_owned(),
        arguments,
    }
}

fn index_resolver(program: &Program) -> Resolver<'_, '_> {
    Resolver {
        function_cells: std::borrow::Cow::Owned(crate::source_verify::function_cells(program)),
        program,
        byte_helpers: crate::loop_calls::byte_writer::SourceIndex::new(program),
        declarations: DeclarationIndex::from_verified(program).expect("declarations index"),
        reuse: None,
        function_work: FunctionResolutionWork::default(),
        fused_producer: None,
        partition_profile: false,
    }
}

type Work = (std::collections::BTreeMap<String, usize>, usize, bool);

fn resolve_with_work(program: &Program) -> (ResolvedProgram, Work) {
    let (resolved, work) = crate::hir::resolve_verified(program, None).unwrap();
    (resolved, (work.costs, work.reused, work.static_text))
}

/// Source-text walk of the thirteen sites for a program without generic
/// functions: entrypoint, types, lifecycles, class methods, interfaces,
/// imports, record-layout owners, and monomorphic function owners.
fn expected_requests(program: &Program) -> Vec<String> {
    let mut texts = Vec::new();
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    texts.push(main.stable_id.clone());
    for declaration in program
        .types
        .iter()
        .chain(crate::prelude::declarations_for_program(program))
    {
        texts.push(declaration.stable_id.clone());
        match &declaration.kind {
            TypeDeclarationKind::Resource { lifecycles } => {
                texts.push(lifecycles[0].stable_id.clone().unwrap());
            }
            TypeDeclarationKind::Class { methods, .. } => {
                texts.extend(methods.iter().map(|method| method.stable_id.clone()));
            }
            _ => {}
        }
    }
    for interface in &program.interfaces {
        texts.push(interface.stable_id.clone());
        texts.extend(
            interface
                .imports
                .iter()
                .map(|import| import.stable_id.clone()),
        );
    }
    for declaration in &program.types {
        if matches!(
            declaration.kind,
            TypeDeclarationKind::Record { .. } | TypeDeclarationKind::Class { .. }
        ) && declaration.type_parameters.is_empty()
        {
            texts.push(declaration.stable_id.clone());
        }
    }
    for function in &program.functions {
        if function.type_parameters.is_empty() {
            texts.push(function.stable_id.clone());
        }
    }
    for declaration in &program.types {
        if let TypeDeclarationKind::Class { methods, .. } = &declaration.kind {
            for method in methods {
                if method.type_parameters.is_empty() {
                    texts.push(method.stable_id.clone());
                }
            }
        }
    }
    texts.sort();
    texts
}

fn held(index: &DeclarationIndex, id: &DeclarationId) {
    let entry = index
        .declarations
        .get(id.as_str())
        .expect("resolved identity names its own index entry");
    assert_eq!(
        id.as_str().as_ptr(),
        entry.id.as_str().as_ptr(),
        "`{id}` is not the index carrier"
    );
}

fn assert_resolved_identities_are_index_carriers(resolved: &ResolvedProgram) {
    assert_reconstructed_identities_are_index_carriers(resolved, |_| true);
}

/// Pointer-sharing claim scoped to identities this pass actually reconstructed:
/// every non-function identity, plus only the functions `reconstructed` selects.
/// Function bodies reused from a prior module legitimately keep their prior
/// carriers, so they are outside this claim.
fn assert_reconstructed_identities_are_index_carriers(
    resolved: &ResolvedProgram,
    reconstructed: impl Fn(&DeclarationId) -> bool,
) {
    let index = &resolved.declarations;
    held(index, &resolved.entrypoint);
    for declaration in &resolved.types {
        held(index, &declaration.id);
        match &declaration.kind {
            ResolvedTypeDeclarationKind::Resource { drop } => {
                held(index, &drop.id);
                if let ResolvedResourceDropKind::Imported { import, .. } = &drop.kind {
                    held(index, import);
                }
            }
            ResolvedTypeDeclarationKind::Class { methods, .. } => {
                methods.iter().for_each(|method| held(index, method));
            }
            _ => {}
        }
    }
    for interface in &resolved.interfaces {
        held(index, &interface.id);
        for import in &interface.imports {
            held(index, &import.id);
            held(index, &import.interface);
        }
    }
    for function in &resolved.functions {
        if reconstructed(&function.id) {
            held(index, &function.id);
        }
    }
    for template in &resolved.function_templates {
        held(index, &template.id);
        for ty in template
            .params
            .iter()
            .map(|parameter| &parameter.ty)
            .chain([&template.return_type])
        {
            held_owners(index, ty);
        }
    }
}

/// Every type-parameter owner nested anywhere in a template signature type,
/// including `Vec<T>` arguments and callable parameters/results.
fn held_owners(index: &DeclarationIndex, ty: &ResolvedType) {
    match ty {
        ResolvedType::TypeParameter { owner, .. } => held(index, owner),
        ResolvedType::Nominal { arguments, .. } => {
            arguments
                .iter()
                .for_each(|argument| held_owners(index, argument));
        }
        ResolvedType::Function { parameters, result } => {
            parameters
                .iter()
                .for_each(|parameter| held_owners(index, parameter));
            held_owners(index, result);
        }
        _ => {}
    }
}

/// Complete successful wire of an admitted fixture; a refusal fails the test.
fn wire(resolved: &ResolvedProgram) -> Vec<u8> {
    crate::cache_codec::encode(resolved).expect("admitted fixture encodes completely")
}

type Paid = (ResolvedProgram, Work, Vec<usize>);

fn paid_work(work: FunctionResolutionWork) -> Work {
    (work.costs, work.reused, work.static_text)
}

/// The paid owned route: explicit identity payer, original verification first.
fn paid_owned(program: &Program) -> Paid {
    let mut reserved = Vec::new();
    let (resolved, work) = crate::hir::identity_resolution::resolve(program, None, |bytes| {
        reserved.push(bytes);
        Ok::<(), ()>(())
    })
    .unwrap();
    (resolved, paid_work(work), reserved)
}

/// The paid checked route with a prior exact module supplied for reuse.
fn paid_checked(
    program: &Program,
    previous: Option<(&ResolvedProgram, &std::collections::BTreeMap<String, usize>)>,
) -> Paid {
    let mut reserved = Vec::new();
    let (resolved, work) = crate::hir::identity_resolution::resolve_checked(
        program,
        previous.map(|_| program),
        previous.map(|(resolved, _)| resolved),
        previous.map(|(_, costs)| costs),
        None,
        |bytes| {
            reserved.push(bytes);
            Ok::<(), ()>(())
        },
    )
    .unwrap();
    (resolved, paid_work(work), reserved)
}

/// The paid borrowed-source route retains the full invariant inventory and rewrite.
fn paid_borrowed(program: &Program) -> Result<Paid, String> {
    let mut reserved = Vec::new();
    crate::hir::identity_resolution::resolve_source(program, None, |bytes| {
        reserved.push(bytes);
        Ok::<(), ()>(())
    })
    .map(|(resolved, work)| (resolved, paid_work(work), reserved))
    .map_err(|failure| format!("{failure:?}"))
}

#[test]
fn resolver_sharing_equals_saved_construction_for_hir_index_graph_wire_and_work() {
    for (source, path, invariant) in FIXTURES {
        let program = crate::check(source, path).unwrap();
        let (old, new) = pair(|| resolve_with_work(&program));
        assert_eq!(old.0, new.0);
        assert_eq!(old.1, new.1);
        // `PartialEq` on the resolved program includes its whole DeclarationIndex.
        assert_eq!(old.0.declarations, new.0.declarations);
        let (old_graph, new_graph) = pair(|| crate::graph::to_json(&program));
        assert_eq!(old_graph, new_graph);
        assert!(new_graph.is_ok());
        // Genuine admitted fixtures must encode: both wires succeed and are
        // byte-for-byte equal; a refusal would fail here, never compare `None`.
        let (old_wire, new_wire) = (wire(&old.0), wire(&new.0));
        assert!(!new_wire.is_empty());
        assert_eq!(old_wire, new_wire);
        // The public verifying route and the borrowed-source route agree too.
        let (old_public, new_public) = pair(|| crate::hir::resolve(&program));
        assert_eq!(old_public, new_public);
        assert!(new_public.is_ok());
        let (old_source, new_source) = pair(|| crate::hir::resolve_source(&program, None));
        let ((old_source, old_work), (new_source, new_work)) =
            (old_source.unwrap(), new_source.unwrap());
        assert_eq!(old_source, old.0);
        assert_eq!(new_source, new.0);
        assert_eq!(old_source, new_source);
        assert_eq!(old_work.costs, new_work.costs);
        assert_eq!(old_work.reused, new_work.reused);
        assert_eq!(old_work.static_text, new_work.static_text);
        if invariant {
            // Both inventory and transformed passes suppress reuse exactly as
            // the complete saved owning route does; neither is a refusal.
            assert_eq!(old_work.reused, 0);
            assert_eq!(new_work.reused, 0);
        }
        // Real paid owned, paid checked, and paid borrowed receivers.
        let (old_paid, new_paid) = pair(|| paid_owned(&program));
        assert_eq!(old_paid, new_paid);
        assert!(!new_paid.2.is_empty());
        let (old_paid, new_paid) = pair(|| paid_checked(&program, None));
        assert_eq!(old_paid, new_paid);
        let (old_paid, new_paid) = pair(|| paid_borrowed(&program));
        let (old_paid, new_paid) = (old_paid.unwrap(), new_paid.unwrap());
        assert_eq!(old_paid, new_paid);
        assert_eq!(wire(&old_paid.0), wire(&new_paid.0));
        if invariant {
            // Independently saved construction is still the owning oracle.
            // Compare the complete identity request sequence across both
            // clause inventory and transformed passes, including real bytes.
            let (old_owned, new_owned) = pair(|| paid_owned(&program));
            assert_eq!(old_paid, old_owned);
            assert_eq!(new_paid, new_owned);
            assert!(!old_paid.2.is_empty());
        }
    }
}

#[test]
fn cache_codec_refusals_are_pinned_exactly_beside_the_successful_codec_owner() {
    for (source, path, _) in FIXTURES {
        let program = crate::check(source, path).unwrap();
        let (old, new) = pair(|| crate::hir::resolve(&program).unwrap());
        // Successful owner: complete bytes, and a decoded program re-encodes
        // to the identical wire under both policies.
        let (old_wire, new_wire) = (wire(&old), wire(&new));
        assert_eq!(old_wire, new_wire);
        let decoded: ResolvedProgram = crate::cache_codec::decode(&new_wire).unwrap();
        assert_eq!(wire(&decoded), new_wire);
        // Intentionally unsupported shape: a one-byte limit cannot hold any
        // program. Complete error equality, never `Ok`-erasing `.ok()`.
        let refused_old = crate::cache_codec::encode_bounded(&old, 1).unwrap_err();
        let refused_new = crate::cache_codec::encode_bounded(&new, 1).unwrap_err();
        assert!(!refused_new.is_empty());
        assert_eq!(refused_old, refused_new);
    }
}

#[test]
fn resolver_sharing_preserves_diagnostics_and_unchanged_verifier_outcomes_for_hostile_text() {
    let program = crate::check(CENSUS, "resolver-held-census.spx").unwrap();
    for text in ["", "nul\0identity", "Røw.雪", "choice.type"] {
        let mut changed = program.clone();
        changed.types[2].stable_id = text.to_owned();
        let (old, new) = pair(|| crate::hir::resolve(&changed));
        assert_eq!(old, new);
        let (old_graph, new_graph) = pair(|| crate::graph::to_json(&changed));
        assert_eq!(old_graph, new_graph);
        // Neither policy grants the changed AST verifier authority.
        assert_eq!(
            crate::verify::verify(&changed),
            crate::verify::verify(&changed.clone())
        );
    }
    // Source-order errors keep their original position under both policies.
    let mut lifecycle = program.clone();
    let TypeDeclarationKind::Resource { lifecycles } = &mut lifecycle.types[0].kind else {
        panic!("Token must be a resource")
    };
    lifecycles[0].stable_id = None;
    let (old, new) = pair(|| crate::hir::resolve(&lifecycle));
    assert_eq!(old, new);
    let mut no_main = program.clone();
    no_main.functions.retain(|function| function.name != "main");
    let (old, new) = pair(|| crate::hir::resolve(&no_main));
    assert_eq!(old, new);
    for source in [
        super::SOURCE.replace("@id(\"child.read\")", "@id(\"base.read\")"),
        super::SOURCE.replace("Child : Base", "Child : Missing"),
        super::SOURCE.replace("class Base {", "class Base : Child {"),
    ] {
        let changed = crate::parse(&source, "resolver-held-classes.spx").unwrap();
        let (old, new) = pair(|| crate::hir::resolve(&changed));
        assert_eq!(old, new);
        assert!(new.is_err());
    }
}

#[test]
fn resolver_identities_are_the_current_index_carriers_and_never_cross_indexes() {
    for (source, path, _) in FIXTURES {
        let program = crate::check(source, path).unwrap();
        let first = crate::hir::resolve(&program).unwrap();
        let second = crate::hir::resolve(&program.clone()).unwrap();
        assert_resolved_identities_are_index_carriers(&first);
        assert_resolved_identities_are_index_carriers(&second);
        assert_eq!(first, second);
        // Equal text from another pass never borrows the earlier carrier.
        assert_ne!(
            first.entrypoint.as_str().as_ptr(),
            second.entrypoint.as_str().as_ptr()
        );
        // The saved policy builds its own carriers, which are the saved
        // result's own index entries only by construction, never shared.
        let (old, _) = saved(false, || crate::hir::resolve(&program).unwrap());
        assert_ne!(
            old.entrypoint.as_str().as_ptr(),
            first.entrypoint.as_str().as_ptr()
        );
        assert_ne!(
            old.entrypoint.as_str().as_ptr(),
            old.declarations.declarations[old.entrypoint.as_str()]
                .id
                .as_str()
                .as_ptr()
        );
    }
}

#[test]
fn held_lookup_is_total_over_every_text_the_resolver_requests() {
    let program = crate::check(CENSUS, "resolver-held-census.spx").unwrap();
    let index = DeclarationIndex::from_verified(&program).unwrap();
    for text in expected_requests(&program) {
        let shared = index.held_identity(&text);
        assert_eq!(shared.as_str(), text);
        assert_eq!(
            shared.as_str().as_ptr(),
            index.declarations[text.as_str()].id.as_str().as_ptr()
        );
    }
    // Every actual Resolver constructor routes through a pass index; this
    // paid route differs only in who built that index's carriers.
    let run = || {
        let mut reserved = Vec::new();
        let outcome = crate::hir::identity_resolution::resolve(&program, None, |bytes| {
            reserved.push(bytes);
            Ok::<(), ()>(())
        });
        match outcome {
            Ok((resolved, work)) => (resolved, work.costs, reserved),
            Err(_) => panic!("paid resolution route refused the census fixture"),
        }
    };
    let (old, new) = pair(run);
    assert_eq!(old, new);
    assert!(!new.2.is_empty());
    assert_resolved_identities_are_index_carriers(&new.0);
}

#[test]
fn resolver_sharing_removes_literal_reconstruction_payloads_through_resolved_program_drop() {
    let program = crate::check(CENSUS, "resolver-held-census.spx").unwrap();
    // Warm lazily initialized static state outside both measurements.
    drop(crate::hir::resolve_verified(&program, None).unwrap());
    // Hand-counted census (prelude identities are read from the declaration
    // slice the resolver itself selects): 202 main; 215 every type; 225 both
    // lifecycles; 291 both class methods; 334/371 the interface and import;
    // 660 the three layout owners; 681 both functions and both class methods.
    let mut literal = vec![
        "app.discard",
        "app.main",
        "app.main",
        "base.read",
        "base.read",
        "base.type",
        "base.type",
        "child.read",
        "child.read",
        "child.type",
        "child.type",
        "choice.type",
        "host.dispose",
        "host.interface",
        "latch.drop",
        "latch.type",
        "row.type",
        "row.type",
        "token.drop",
        "token.type",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    literal.extend(
        crate::prelude::declarations_for_program(&program)
            .iter()
            .map(|declaration| declaration.stable_id.clone()),
    );
    literal.sort();
    let (_, mut recorded) = saved(true, || {
        drop(crate::hir::resolve_verified(&program, None).unwrap())
    });
    recorded.sort();
    assert_eq!(recorded, literal);
    assert_eq!(recorded, expected_requests(&program));
    let (_, old) = measure(|| {
        saved(false, || {
            drop(crate::hir::resolve_verified(&program, None).unwrap())
        })
    });
    assert_eq!(requests(), literal.len());
    let (_, new) = measure(|| drop(crate::hir::resolve_verified(&program, None).unwrap()));
    // Two requests per old reconstruction: the source String and the backing.
    assert_eq!(old.allocations, new.allocations + 2 * literal.len());
    assert_eq!(old.reallocations, new.reallocations);
    assert_eq!(old.allocated, new.allocated + removed_bytes(&literal));
    // Whole inequality controls: only formerly unpaid reconstructions vanish.
    assert!(old.allocations > new.allocations);
    assert!(old.allocated > new.allocated);
    for observed in [old, new] {
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}

#[test]
fn invariant_inventory_and_desugared_passes_each_share_only_their_own_index() {
    let program = crate::check(INVARIANT, "resolver-held-invariant.spx").unwrap();
    drop(crate::hir::resolve_verified(&program, None).unwrap());
    let (_, mut recorded) = saved(true, || {
        drop(crate::hir::resolve_verified(&program, None).unwrap())
    });
    recorded.sort();
    // Entry point and monomorphic owner per pass; record type and layout owner
    // per pass. The inventory and desugared programs are distinct ASTs.
    for (text, passes) in [("app.main", 2usize), ("gate.type", 2)] {
        let count = recorded.iter().filter(|recorded| *recorded == text).count();
        assert!(count >= 2 * passes, "{text}: {count}");
    }
    let (_, old) = measure(|| {
        saved(false, || {
            drop(crate::hir::resolve_verified(&program, None).unwrap())
        })
    });
    let (_, new) = measure(|| drop(crate::hir::resolve_verified(&program, None).unwrap()));
    assert_eq!(requests(), recorded.len());
    assert_eq!(old.allocations, new.allocations + 2 * recorded.len());
    assert_eq!(old.allocated, new.allocated + removed_bytes(&recorded));
    for observed in [old, new] {
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}

#[test]
fn repeated_generic_owner_queries_share_one_carrier_and_remove_exact_reconstructions() {
    let program = crate::check(GENERIC, "resolver-held-generic.spx").unwrap();
    let resolver = index_resolver(&program);
    let template = program
        .functions
        .iter()
        .find(|function| function.name == "identity")
        .unwrap();
    let span = Span::default();
    let parameter = named("T", Vec::new());
    // `Vec<T>` and `fn(T)->T` are admitted for the real `map` template; both
    // queries are explicit successes, never a skipped `if let Ok`.
    let callable_program = crate::check(CALLABLE, "resolver-held-callable.spx").unwrap();
    let callable_resolver = index_resolver(&callable_program);
    let map = callable_program
        .functions
        .iter()
        .find(|function| function.name == "map")
        .unwrap();
    let wrapped = named("Vec", vec![parameter.clone()]);
    let callback = Type::Function {
        parameters: vec![parameter.clone()],
        result: Box::new(parameter.clone()),
    };
    let query =
        |resolver: &Resolver<'_, '_>, template: &crate::ast::Function, ty: &Type, rounds: usize| {
            for _ in 0..rounds {
                drop(resolver.resolve_function_type(template, ty, span));
            }
        };
    let entry = &resolver.declarations.declarations["identity"].id;
    for _ in 0..8 {
        let ResolvedType::TypeParameter { owner, index: 0 } = resolver
            .resolve_function_type(template, &parameter, span)
            .unwrap()
        else {
            panic!("`T` resolves to the template's first type parameter")
        };
        assert_eq!(owner.as_str().as_ptr(), entry.as_str().as_ptr());
    }
    let map_entry = &callable_resolver.declarations.declarations["adapter.map"].id;
    let ResolvedType::Nominal { arguments, .. } = callable_resolver
        .resolve_function_type(map, &wrapped, span)
        .unwrap()
    else {
        panic!("`Vec<T>` resolves to a nominal type")
    };
    let [ResolvedType::TypeParameter { owner, index: 0 }] = arguments.as_slice() else {
        panic!("`Vec<T>` keeps one type-parameter argument")
    };
    assert_eq!(owner.as_str().as_ptr(), map_entry.as_str().as_ptr());
    let ResolvedType::Function { parameters, result } = callable_resolver
        .resolve_function_type(map, &callback, span)
        .unwrap()
    else {
        panic!("`fn(T)->T` resolves to a callable type")
    };
    let (
        [ResolvedType::TypeParameter { owner: input, .. }],
        ResolvedType::TypeParameter { owner: output, .. },
    ) = (parameters.as_slice(), &*result)
    else {
        panic!("`fn(T)->T` keeps type-parameter input and result")
    };
    assert_eq!(input.as_str().as_ptr(), map_entry.as_str().as_ptr());
    assert_eq!(output.as_str().as_ptr(), map_entry.as_str().as_ptr());
    for ty in [&wrapped, &callback] {
        let (old_result, new_result) =
            pair(|| callable_resolver.resolve_function_type(map, ty, span));
        assert!(new_result.is_ok());
        assert_eq!(old_result, new_result);
    }
    // Warm thread-local policy state, then count literal reconstructions: one
    // per `T`; `Vec<T>` reconstructs for its argument and for itself; the
    // callable reconstructs for its one parameter, its result and the generic
    // callback-signature owner check.
    query(&resolver, template, &parameter, 1);
    for (resolver, template, ty, per_query, text) in [
        (&resolver, template, &parameter, 1usize, "identity"),
        (&callable_resolver, map, &wrapped, 2, "adapter.map"),
        (&callable_resolver, map, &callback, 3, "adapter.map"),
    ] {
        query(resolver, template, ty, 1);
        let (_, old) = measure(|| saved(false, || query(resolver, template, ty, 8)));
        assert_eq!(requests(), 8 * per_query);
        let (_, new) = measure(|| query(resolver, template, ty, 8));
        let reconstructions = 8 * per_query;
        assert_eq!(
            old.allocations,
            new.allocations + 2 * reconstructions,
            "owner {text}, {per_query} reconstructions per query: old={old:?}, current={new:?}"
        );
        assert_eq!(old.reallocations, new.reallocations);
        assert_eq!(
            old.allocated,
            new.allocated + reconstructions * (text.len() + backing(text.len()))
        );
        assert!(old.allocated > new.allocated);
        for observed in [old, new] {
            assert!(!observed.invalid);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
        }
    }
    // Argument and result profile owners (sites 116 and 126) request only the
    // selected template's text and keep the original boolean/diagnostic.
    let caller = FunctionExecutionId::Monomorphic(resolver.declarations.held_identity("zed"));
    let admitted = || {
        resolver.generic_function_arguments_are_admitted(&caller, template, &[ResolvedType::I64])
    };
    let ((old_admitted, texts), new_admitted) = (saved(true, admitted), admitted());
    assert!(new_admitted.is_ok());
    assert_eq!(old_admitted, new_admitted);
    assert!(texts.len() >= 2, "{texts:?}");
    assert!(texts.iter().all(|text| text == "identity"), "{texts:?}");
}

#[test]
fn callable_and_closure_fixtures_reach_repeated_owner_sites_with_exact_payload_removal() {
    // `(source, path, owner, least requests for that owner)`. Lower bounds are
    // derived from signatures alone, each `T` costing one and each `Vec<T>`
    // two: `map<T>(Vec<T>, fn(T)->T)->Vec<T>` is 2 + 2 + 2 + template identity
    // (1); `filter` is 2 + 1 + 2 + 1; `fold<T>(Vec<T>, T, fn(T,T)->T)->T` is
    // 2 + 1 + 3 + 1 + 1; `fill<T>(Vec<T>, T)->Vec<T>` is 2 + 1 + 2 + 1, plus
    // the closure's own `fn(T)->T` signature through the closure resolver.
    for (source, path, owner, least) in [
        (
            CALLABLE,
            "resolver-held-callable.spx",
            "adapter.map",
            7usize,
        ),
        (CALLABLE, "resolver-held-callable.spx", "adapter.filter", 6),
        (CALLABLE, "resolver-held-callable.spx", "adapter.fold", 8),
        (
            GENERIC_CLOSURE,
            "resolver-held-generic-closure.spx",
            "gc.fill",
            6,
        ),
        (CLOSURES, "resolver-held-closures.spx", "snapshot", 1),
        (CLOSURES, "resolver-held-closures.spx", "shadow", 1),
    ] {
        let program = crate::check(source, path).unwrap();
        drop(crate::hir::resolve_verified(&program, None).unwrap());
        let (resolved, recorded) = saved(true, || {
            let (resolved, _) = crate::hir::resolve_verified(&program, None).unwrap();
            resolved
        });
        // Positive controls: the admitted program really contains the shapes.
        match owner {
            "gc.fill" | "adapter.map" => assert!(!resolved.function_templates.is_empty()),
            _ => assert!(!resolved.functions.is_empty()),
        }
        let count = recorded.iter().filter(|text| *text == owner).count();
        assert!(count >= least, "{owner}: {count} < {least}");
        drop(resolved);
        let (_, old) = measure(|| {
            saved(false, || {
                drop(crate::hir::resolve_verified(&program, None).unwrap())
            })
        });
        assert_eq!(requests(), recorded.len());
        let (_, new) = measure(|| drop(crate::hir::resolve_verified(&program, None).unwrap()));
        assert_eq!(old.allocations, new.allocations + 2 * recorded.len());
        assert_eq!(old.reallocations, new.reallocations);
        assert_eq!(old.allocated, new.allocated + removed_bytes(&recorded));
        assert!(old.allocated > new.allocated);
        for observed in [old, new] {
            assert!(!observed.invalid);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
        }
    }
}

#[test]
fn checked_function_reuse_keeps_prior_carriers_and_saved_costs_with_old_new_parity() {
    for (source, path) in [
        (CENSUS, "resolver-held-census.spx"),
        (super::SOURCE, "resolver-held-classes.spx"),
        (CLOSURES, "resolver-held-closures.spx"),
    ] {
        let program = crate::check(source, path).unwrap();
        let (previous, work) =
            crate::hir::resolve_with_function_reuse(&program, None, None, None, None).unwrap();
        let costs = work.costs;
        let run = || {
            let (resolved, work) = crate::hir::resolve_with_function_reuse(
                &program,
                Some(&program),
                Some(&previous),
                Some(&costs),
                None,
            )
            .unwrap();
            (resolved, paid_work(work))
        };
        // Independent saved-old versus actual-new: complete HIR (including the
        // whole index), saved costs, reuse count, static-text flag and wire.
        let (old, new) = pair(run);
        assert_eq!(old, new);
        assert!(new.1 .1 > 0, "checked reuse must reuse a function body");
        assert_eq!(new.1 .0, costs);
        assert_eq!(new.0, previous);
        assert_eq!(wire(&old.0), wire(&new.0));
        // Prior cached carriers are retained as they always were: every reused
        // top-level function keeps its previous carrier in both policies and
        // never points at the new index. Nothing is rebound.
        for resolved in [&old.0, &new.0] {
            for function in &program.functions {
                let prior = previous
                    .functions
                    .iter()
                    .find(|candidate| candidate.id.as_str() == function.stable_id)
                    .unwrap();
                let current = resolved
                    .functions
                    .iter()
                    .find(|candidate| candidate.id.as_str() == function.stable_id)
                    .unwrap();
                assert_eq!(current.id.as_str().as_ptr(), prior.id.as_str().as_ptr());
                assert_ne!(
                    current.id.as_str().as_ptr(),
                    resolved.declarations.declarations[function.stable_id.as_str()]
                        .id
                        .as_str()
                        .as_ptr()
                );
            }
        }
        // Only the optimized policy reconstructs identities from this pass's
        // index. The saved policy deliberately keeps its fresh carriers.
        let methods: Vec<&DeclarationId> = new
            .0
            .types
            .iter()
            .flat_map(|declaration| match &declaration.kind {
                ResolvedTypeDeclarationKind::Class { methods, .. } => methods.iter(),
                _ => [].iter(),
            })
            .collect();
        assert_reconstructed_identities_are_index_carriers(&new.0, |id| methods.contains(&id));
        // The same reuse through the paid checked route: identical requests,
        // output and reuse, with and without a prior module.
        let (old_paid, new_paid) = pair(|| paid_checked(&program, Some((&previous, &costs))));
        assert_eq!(old_paid, new_paid);
        assert!(new_paid.1 .1 > 0);
        assert_eq!(new_paid.2, paid_checked(&program, None).2);
        // Entire receiver-to-Drop work: only the formerly unpaid
        // reconstructions vanish; reused clones and saved costs are unchanged.
        drop(run());
        let (_, recorded) = saved(true, || drop(run()));
        assert!(!recorded.is_empty());
        let (_, old_drop) = measure(|| saved(false, || drop(run())));
        assert_eq!(requests(), recorded.len());
        let (_, new_drop) = measure(|| drop(run()));
        assert_eq!(
            old_drop.allocations,
            new_drop.allocations + 2 * recorded.len()
        );
        assert_eq!(old_drop.reallocations, new_drop.reallocations);
        assert_eq!(
            old_drop.allocated,
            new_drop.allocated + removed_bytes(&recorded)
        );
        for observed in [old_drop, new_drop] {
            assert!(!observed.invalid);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
        }
    }
}


/// The inference target is selected from the same source/index by the ordinary
/// call resolver. Reusing its exact held identity removes reconstruction only;
/// the full argument inference and instance constructor remain the same.
#[test]
fn inference_and_call_signature_keep_complete_old_results_and_index_backing_through_drop() {
    let program = crate::check(GENERIC, "resolver-held-generic.spx").unwrap();
    let resolver = index_resolver(&program);
    let target = program
        .functions
        .iter()
        .find(|function| function.stable_id == "identity")
        .unwrap();
    let caller = FunctionExecutionId::Monomorphic(resolver.declarations.held_identity("app.main"));
    let arguments = [crate::ast::Expr {
        kind: crate::ast::ExprKind::Int(7),
        span: Span::default(),
    }];
    let bindings = std::collections::BTreeMap::new();
    let run = || {
        let inferred = resolver.infer_call_arguments(&caller, target, &arguments, &bindings);
        let signature = resolver.ordinary_call_signature(
            &caller,
            target,
            &[Type::I64],
            &arguments,
            &bindings,
            Span::default(),
        );
        (inferred, signature)
    };
    let (old, current) = pair(run);
    assert_eq!(old, current);
    assert_eq!(current.0, Some(vec![Type::I64]));
    assert!(current.1.is_ok());
    drop((old, current));
    let (_, recorded) = saved(true, || drop(run()));
    assert!(
        recorded
            .iter()
            .filter(|text| text.as_str() == "identity")
            .count()
            >= 2
    );
    let (_, old) = measure(|| saved(false, || drop(run())));
    assert_eq!(requests(), recorded.len());
    let (_, current) = measure(|| drop(run()));
    assert_eq!(old.allocations, current.allocations + 2 * recorded.len());
    assert_eq!(old.reallocations, current.reallocations);
    assert_eq!(old.allocated, current.allocated + removed_bytes(&recorded));
    for observed in [old, current] {
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
    // No new identity allowance is requested after this index was constructed.
    // Exhausting an unrelated ambient output budget does not invalidate it.
    let ((inferred, before, after), refused, spent) =
        crate::bounded_output::with_limit_usage(0, || {
            let before = crate::bounded_output::active_remaining();
            let inferred = resolver.infer_call_arguments(&caller, target, &arguments, &bindings);
            (inferred, before, crate::bounded_output::active_remaining())
        });
    assert_eq!(inferred, Some(vec![Type::I64]));
    assert!(!refused);
    assert_eq!(spent, 0);
    assert_eq!(before, after);

    // Exercise the actual ordinary-call dispatcher and its recursive evidence
    // path, not only the direct test wrappers above. Both calls omit their type
    // arguments; the inner result supplies the outer generic argument.
    assert_eq!(GENERIC.matches("identity<i64>(alpha())").count(), 1);
    let nested_source = GENERIC.replace("identity<i64>(alpha())", "identity(identity(alpha()))");
    let nested_program = crate::check(&nested_source, "resolver-held-nested-inference.spx").unwrap();
    let (nested_old, nested_current) = pair(|| paid_owned(&nested_program));
    // Includes the complete ResolvedProgram, all work fields, and the ordered
    // original identity-payer requests, under the saved allocation policy.
    assert_eq!(nested_old, nested_current);
    assert_eq!(wire(&nested_old.0), wire(&nested_current.0));
    assert_resolved_identities_are_index_carriers(&nested_current.0);
    assert!(nested_current.0.function_instances.iter().any(|instance| {
        instance.template.as_str() == "identity"
            && instance.type_arguments == vec![ResolvedType::I64]
    }));
    drop((nested_old, nested_current));
    // The whole real paid resolver and all returned roots live inside each
    // observation. This is the complete saved construction-policy comparison,
    // not a claim that only the two new sites account for its total saving.
    let (_, nested_old_cost) = measure(|| drop(saved(false, || paid_owned(&nested_program))));
    let (_, nested_current_cost) = measure(|| drop(paid_owned(&nested_program)));
    assert!(nested_old_cost.allocations > nested_current_cost.allocations);
    assert!(nested_old_cost.allocated > nested_current_cost.allocated);
    for observed in [nested_old_cost, nested_current_cost] {
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}

#[test]
fn inference_index_payment_refuses_exact_short_and_preserves_original_payer_before_lowering() {
    let program = crate::check(GENERIC, "resolver-held-generic.spx").unwrap();
    let reference = paid_owned(&program);
    let bill = reference.2.iter().sum::<usize>();
    assert!(bill > 0);
    for limit in [0, bill - 1, bill] {
        let mut remaining = limit;
        // The request recorder is fixture storage, acquired before observation.
        let mut requested = Vec::with_capacity(reference.2.len());
        let ((accepted, identity_refusal), observed) = measure(|| {
            let result = crate::hir::identity_resolution::resolve(&program, None, |bytes| {
                requested.push(bytes);
                if bytes > remaining {
                    return Err(bytes);
                }
                remaining -= bytes;
                Ok(())
            });
            match result {
                Ok((resolved, work)) => {
                    assert_eq!(resolved, reference.0);
                    assert_eq!(paid_work(work), reference.1);
                    drop(resolved);
                    (true, None)
                }
                Err(crate::hir::identity_resolution::Failure::Identity(bytes)) => {
                    (false, Some(bytes))
                }
                Err(other) => panic!("unexpected semantic refusal: {other:?}"),
            }
        });
        assert_eq!(accepted, limit == bill);
        if limit == bill {
            assert_eq!(remaining, 0);
            assert_eq!(requested, reference.2);
            assert!(identity_refusal.is_none());
        } else {
            assert_eq!(identity_refusal, requested.last().copied());
            assert!(identity_refusal.unwrap() > remaining);
            assert!(reference.2.starts_with(&requested));
        }
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}
