# claude review (custody-ast-drop-frontier)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

**No definite blockers found.** I checked this by reading only, with no tools and no test runs.

**Compile and path**
- `Stack<T>` has no `Drop` impl, so there are no partial-move problems. `Extend<T>` is the only `extend` source, so `types.extend(..)` and `Stack::new()` infer without ambiguity.
- The `#[path]` attributes resolve relative to `src/` for `ast/drop_frontier.rs` and `ast/drop_original.rs`. The nested `drop_original_tests.rs` resolves relative to `src/ast/`.
- `use super::*` in `drop_original.rs` and its tests picks up `mem` and the AST types. The lexical `fn drop(Program)` shadows the prelude `drop` only in those two modules.
- The `Original(program)` constructor is `pub(super)`, which reaches `iterative_program_drop_tests` as a descendant of `ast`.
- `Original::drop` reaches `self.functions` and the other fields through `DerefMut`, and the `&mut Program` borrows end before the field glue runs.

**Stack LIFO**
- The spill is non-empty only after the inline array is full.
- `pop` drains the spill before inline, and `push` refills inline only when the spill is empty.
- So the order is identical to a single `Vec`.
- The spill-capacity test is deterministic. A `usize` Vec first grows to capacity 4, and the six re-pushes put exactly 4 into the spill.

**Destruction order**
- `Program::drop` has the same local declaration order as the original, so locals drop in the same reverse order.
- The stacks are drained in the same sequence. `types` is drained last, and nothing pushes into it after that point.
- Normal and wrapper paths free in the same order. `Original::drop` runs first and empties all recursive roots. The empty inner `Program::drop` then runs, allocating nothing because the stacks are inline. The remaining fields drop in declaration order.
- The expected free order in the order test is function ids (first, second, third), then bodies (third, second, first), then `path`, then `module`. That is the same sequence for both implementations.

**Tests**
- Inline stacks acquire no heap for the small program.
- The original arm allocates through its `Vec` worklists, and the wide arm allocates through the spill.
- The free-order watch has 8 distinct pointers, within the 32 limit.

**Limitations**
- Spill growth is an ordinary `Vec` allocation. It is unpriced and unrefusable, and an allocator failure aborts. That matches the stated boundary.
- `implementations` and `session_protocols` are not traversed. Both versions leave them to field glue, so this is unchanged from the old behavior.
- I could not check the source-inverse assertions, which need the `.before.txt` evidence files and the exact `ast.rs` registration block. I only confirmed the indentation and the substitution logic are consistent.
