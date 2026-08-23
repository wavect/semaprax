# RFC: String + Object-Oriented Types — Large Implementation Badges

Status: Draft — feat/string-oo-types branch
Author: Muse (isolated worktree .agent-worktrees/string-oo)
Date: 2026-08-23

## Goals

Add heap-owned `String` and a minimal but complete OO system (classes, single inheritance, interfaces, `impl` dispatch) to SEMAPRAX without breaking invariants:

- deterministic canonical formatting & graph revisions
- ownership errors are diagnostics, never backend accidents
- safe programs have equivalent behavior on native C11 and Wasm
- public declarations keep persistent @id; expression IDs are revision-scoped

## Non-goals (deferred)

- generics over String/classes (closed initially)
- operator overloading, generics variance, multiple inheritance
- GC, reference counting beyond simple owned drop
- reflection, dynamic loading

## Badge Decomposition (parallelizable)

| Badge | Scope | Key files | Parallel lane |
|-------|-------|-----------|---------------|
| 1. String Core | `Type::String`, string literal HIR, `format`, `verify`, `graph` | ast.rs, lexer.rs, parser.rs, format.rs, hir.rs, verify.rs, graph.rs | Lane A |
| 2. String Runtime | owned String ownership, cleanup, codegen C11/Wasm, interpreter, layouts | aggregate_layout.rs, cleanup_plan.rs, codegen.rs, wasm.rs, interpreter.rs | Lane A' (depends on 1) |
| 3. Class Declarations | `class Name { fields, methods }`, constructor, method call `obj.method()` | ast.rs, parser.rs, format.rs, hir.rs, graph.rs | Lane B |
| 4. Inheritance | `class Child : Parent { }`, `super`, override checking, layout extension | hir.rs, aggregate_layout.rs, codegen.rs, wasm.rs, verify.rs | Lane B' (depends on 3) |
| 5. Interfaces | `interface I { fn foo(); }`, `class C : I { impl }`, interface dispatch | ast.rs..hir.rs..graph.rs..codegen | Lane C |
| 6. Integration | examples, tests, docs, quality gates | tests/*, examples/*, docs/* | Merge gate |

Lane B/C can start AST/parser work in parallel with Lane A after Badge 1 AST land; runtime parts join after.

## String Design (Badge 1+2)

- **Type**: `Type::String` as distinct nominal-like primitive, non-Copy, owned, needs_drop=true.
- **Literal**: `TokenKind::String(String)` already exists; promote to `ExprKind::String(String)` with deduplicated storage. Parser already handles `"` tokens — need to add production for literal expression.
- **Operations** (intrinsics as free functions to avoid method syntax initially):
  - `string_len(s: String) -> i64` borrowed view (or own?)
  - `string_concat(a: String, b: String) -> String`
  - `string_eq(a: String, b: String) -> bool` (via `==` overload or dedicated)
  - `string_empty() -> String`
- **Ownership**: String is moved by default; `borrow String` for read-only. Cleanup inventory drops via `free`.
- **Layouts**: Native64: pointer+len+cap struct; Wasm32: linear memory with allocator shim. For v1 we lower to C string helper (`strdup`, `strlen`, `strcmp`) and wasm imported `string_*` helpers.
- **Canonical format**: `"…"` with same escapes as lexer; round-trip stable.
- **Graph**: `"kind":"string"` leaf.
- **Diagnostics**: SPX-Txxx for mixing String with numeric ops, SPX-Uxxx for move-use-after.

## Class Design (Badge 3)

```spx
@id("demo.point")
class Point {
  @id("demo.point.x") x: i64,
  @id("demo.point.y") y: i64,

  @id("demo.point.new")
  fn new(x: i64, y: i64) -> Point {
    Point { x: x, y: y }
  }

  @id("demo.point.dist")
  fn dist(self: Point) -> i64 { self.x + self.y }
}

@id("demo.main")
fn main() -> i64 {
  let p = Point::new(1,2);
  p.dist()
}
```

- New `TypeDeclarationKind::Class { fields, methods, superClass? }`
- Methods are scoped to class; first param `self: Class` or `self: borrow Class`.
- Call syntax: `receiver.method(args)` lowered to `Class_method(receiver, args)` plus static dispatch.
- `format::canonical` renders class block deterministically.

## Inheritance (Badge 4)

```spx
@id("demo.animal") class Animal { @id("demo.animal.name") name: String, fn speak(self: Animal) -> String { self.name } }
@id("demo.dog") class Dog : Animal { @id("demo.dog.breed") breed: String, fn speak(self: Dog) -> String { string_concat(super.speak(), self.breed) } }
```

- `class Child : Parent { }` ; single inheritance only; cycle rejected SPX-T.
- Layout = Parent fields followed by child fields (aggregate_layout extension).
- Method override requires identical signature; `super` calls parent impl.
- `is` / `as` checks deferred; for v1 assignment `Dog -> Animal` is implicit upcast via slicing copy (own semantics).

## Interfaces (Badge 5)

```spx
@id("demo.shape") interface Shape { @id("demo.shape.area") fn area(self: Shape) -> i64; }
@id("demo.circle") class Circle : Shape { @id("demo.circle.r") r: i64, fn area(self: Circle) -> i64 { self.r * self.r } }
```

- Reuse `interface` keyword: method set with signatures.
- `class C : InterfaceA, InterfaceB` or `class C : Parent : Interface`? Keep single parent + N interfaces: `class C : Parent implements I, J`
- Verifier checks `implements` completeness; missing method -> SPX-T.
- Dispatch: static monomorphized for now (no vtable); interface value = concrete class value, method call `shape.area()` resolves to concrete impl via type of receiver.

## Worktree & Parallelism

- Base worktree: `.agent-worktrees/string-oo` branch `feat/string-oo-types` branched from main 8debcf0, up-to-date.
- Sub-worktrees (for subagents):
  - `.agent-worktrees/string-oo-lane-a` -> String core+runtime
  - `.agent-worktrees/string-oo-lane-b` -> Class
  - `.agent-worktrees/string-oo-lane-c` -> Interfaces
- Each lane branches from `feat/string-oo-types`; merges via `git merge --no-ff` after quality gates.
- Subagents coordinate via this RFC and TODO list.

## Quality Gates per Badge

- parser + canonical round-trip test
- hir + verify diagnostics stable code tests
- graph JSON golden tests
- native C11 O0/O2 + Wasm equivalence for corpus (examples/string_oo.spx)
- `cargo test --locked` + `cargo clippy` + `scripts/quality.sh`

## Sequencing

1. Land Badge 1 AST/parser/format (blocks others' runtime).
2. Lanes A/B/C proceed in parallel on own sub-worktrees.
3. Daily merge to `feat/string-oo-types`.
4. Final Badge 6 integration runs full gate suite.

