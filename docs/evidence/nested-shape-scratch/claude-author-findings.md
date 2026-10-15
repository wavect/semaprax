# claude author findings (nested-shape-scratch)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Nothing was compiled or run, as requested, so the diff below is unverified. Hunks are anchored by context rather than line numbers, because I don't have the file offsets.

## Design

- `Scratch` lives in an inline child `mod scratch` and holds exactly two `Vec`s, `frames` and `shapes`. `CaseMeta` and `Frame` move into that module with their definitions unchanged.
- `derive` keeps its exact signature and behaviour. It builds `Scratch::default()` and calls `derive_reusing`, which takes one extra `&mut Scratch`.
- A `Lease` guard clears both vectors in `Drop`, so success, `?` failure and panic unwind all leave empty payloads. `clear()` keeps the capacity, and the caller's `Scratch` owns the backing until it is dropped. There is no `mem::take`, shrink, refund, cap, cache or summary.
- `reserve_clone`, `reserve_vec`, `push` and `array` are called in the same places and the same order as before.
- `Budget`, `next_flag` and the per-root `Budget::default()` stay per call. Body validation is untouched.

## Diff: paid receiver

```diff
 use super::*;
 use crate::cleanup_plan::replay::ordinary_inventory::{
     storage::{array, push},
     substitute,
 };
+
+mod scratch {
+    use super::*;
+
+    pub(super) struct CaseMeta {
+        pub(super) case: DeclarationId,
+        pub(super) case_index: u32,
+        pub(super) fields: Vec<(DeclarationId, u32)>,
+    }
+    pub(super) enum Frame {
+        Enter(ResolvedType, usize),
+        FinishRecord(DeclarationId, Vec<(DeclarationId, u32)>),
+        FinishVariant(DeclarationId, Vec<CaseMeta>),
+    }
+
+    #[derive(Default)]
+    pub(in super::super) struct Scratch {
+        frames: Vec<Frame>,
+        shapes: Vec<FieldLivenessShape>,
+    }
+
+    pub(super) struct Lease<'a>(&'a mut Scratch);
+
+    impl Scratch {
+        pub(super) fn lease(&mut self) -> Lease<'_> {
+            Lease(self)
+        }
+    }
+
+    impl Lease<'_> {
+        pub(super) fn parts(&mut self) -> (&mut Vec<Frame>, &mut Vec<FieldLivenessShape>) {
+            (&mut self.0.frames, &mut self.0.shapes)
+        }
+    }
+
+    impl Drop for Lease<'_> {
+        fn drop(&mut self) {
+            self.0.frames.clear();
+            self.0.shapes.clear();
+        }
+    }
+}
+
+pub(super) use scratch::Scratch;
+use scratch::{CaseMeta, Frame};
 
 pub(super) fn derive(
     program: &dyn crate::cleanup::ReplayContext,
     function: &ResolvedFunction,
     root: &ResolvedType,
     next_flag: &mut u32,
     depth: usize,
     budget: &mut Budget,
     physical: &mut ReplayBudget,
 ) -> Result<FieldLivenessShape, Diagnostic> {
-    struct CaseMeta {
-        case: DeclarationId,
-        case_index: u32,
-        fields: Vec<(DeclarationId, u32)>,
-    }
-    enum Frame {
-        Enter(ResolvedType, usize),
-        FinishRecord(DeclarationId, Vec<(DeclarationId, u32)>),
-        FinishVariant(DeclarationId, Vec<CaseMeta>),
-    }
-
+    derive_reusing(
+        program,
+        function,
+        root,
+        next_flag,
+        depth,
+        budget,
+        physical,
+        &mut Scratch::default(),
+    )
+}
+
+#[allow(clippy::too_many_arguments)]
+pub(super) fn derive_reusing(
+    program: &dyn crate::cleanup::ReplayContext,
+    function: &ResolvedFunction,
+    root: &ResolvedType,
+    next_flag: &mut u32,
+    depth: usize,
+    budget: &mut Budget,
+    physical: &mut ReplayBudget,
+    scratch: &mut Scratch,
+) -> Result<FieldLivenessShape, Diagnostic> {
     physical.reserve_clone(function, root, "independent shape root type")?;
-    let mut frames = Vec::new();
-    physical.reserve_vec(function, &mut frames, 1, "independent shape frames")?;
+    let mut lease = scratch.lease();
+    let (frames, shapes) = lease.parts();
+    physical.reserve_vec(function, frames, 1, "independent shape frames")?;
     frames.push(Frame::Enter(root.clone(), depth));
-    let mut shapes = Vec::<FieldLivenessShape>::new();
     while let Some(frame) = frames.pop() {
```

The eight `push(&mut shapes, …)` call sites and the two remaining `reserve_vec(&mut frames, …)` sites change only by dropping `&mut`:

```diff
                     push(
-                        &mut shapes,
+                        shapes,
                         FieldLivenessShape::NoDrop,
```
```diff
                     push(
-                        &mut shapes,
+                        shapes,
                         FieldLivenessShape::Leaf {
                             flag,
```
```diff
                     charge_leaf(function, budget)?;
                     push(
-                        &mut shapes,
+                        shapes,
                         FieldLivenessShape::Leaf {
                             flag: next_flag_id(function, next_flag)?,
                             lifecycle: DeclarationId::paid_borrowed(
                                 crate::cleanup::VEC_DROP_LIFECYCLE_ID,
```
```diff
                     push(
-                        &mut shapes,
+                        shapes,
                         FieldLivenessShape::Leaf {
                             flag: next_flag_id(function, next_flag)?,
                             lifecycle: DeclarationId::paid_borrowed(
                                 crate::cleanup::ITER_DROP_LIFECYCLE_ID,
```
```diff
                     push(
-                        &mut shapes,
+                        shapes,
                         FieldLivenessShape::Leaf {
                             flag: next_flag_id(function, next_flag)?,
                             lifecycle: DeclarationId::paid_borrowed(
                                 crate::cleanup::BOX_DROP_LIFECYCLE_ID,
```
```diff
                         push(
-                            &mut shapes,
+                            shapes,
                             FieldLivenessShape::Leaf {
                                 flag: next_flag_id(function, next_flag)?,
                                 lifecycle: drop.id.clone(),
```
```diff
                         physical.reserve_vec(
                             function,
-                            &mut frames,
+                            frames,
                             fields.len().saturating_add(1),
                             "independent record frames",
```
```diff
                         physical.reserve_vec(
                             function,
-                            &mut frames,
+                            frames,
                             entries.len().saturating_add(1),
                             "independent variant frames",
```
```diff
                 push(
-                    &mut shapes,
+                    shapes,
                     FieldLivenessShape::Record {
```
```diff
                 push(
-                    &mut shapes,
+                    shapes,
                     FieldLivenessShape::Variant {
```

## Diff: wrapper

`expected_shape_for_type_paid` is unchanged. Add:

```diff
+pub(super) fn expected_shape_for_type_paid_reusing(
+    program: &dyn crate::cleanup::ReplayContext,
+    function: &ResolvedFunction,
+    ty: &ResolvedType,
+    next_flag: &mut u32,
+    physical: &mut ReplayBudget,
+    scratch: &mut paid::Scratch,
+) -> Result<FieldLivenessShape, Diagnostic> {
+    paid::derive_reusing(
+        program,
+        function,
+        ty,
+        next_flag,
+        0,
+        &mut Budget::default(),
+        physical,
+        scratch,
+    )
+}
```

A caller creates one `paid::Scratch::default()` outside the inventory slot loop and passes `&mut` to it for each slot.

## Concrete obstacles

1. **`reserve_bytes0` is not in the supplied text.** I could not authenticate it, and the diff never calls it. If it matters for the zero-size case, I need its definition.
2. **`ReplayBudget::reserve_vec`, `push` and `array` are unseen.**
   - If `reserve_vec` charges relative to existing capacity, a reused `Scratch` is only charged for growth. In that case, pair each `Scratch` with a single `ReplayBudget`. Otherwise later budgets get capacity they never paid for.
   - If it charges the full request regardless of capacity, reuse is merely conservative.
   - Dropping `&mut` assumes `reserve_vec` takes `&mut Vec<T>` explicitly, so `frames` is implicitly reborrowed. If it is generic over the container, `frames` would be moved and need `&mut *frames`.
3. **The inventory slot loop is not shown.** I could add the entry points but not wire the reuse into the loop.
4. **Visibility is the one change to the moved types.** `CaseMeta` and `Frame` are now `pub(super)`, and `CaseMeta`'s fields are `pub(super)`, because `derive_reusing` constructs and reads them from the parent module. Names, fields and variants are otherwise identical.
5. **`derive_reusing` has 8 parameters,** which trips `clippy::too_many_arguments`. I added a targeted `#[allow]`. `derive` itself stays at 7.
6. **Clearing happens in `Lease::drop`, not at function end.** It still runs after the result value is built, so the returned shape is already moved out. A panic also clears via unwind, and the capacity is retained either way.
7. **Re-export visibility.** `Scratch` is `pub(in super::super)` so `pub(super) use` can hand it to the wrapper's module. This assumes the wrapper's module is the direct parent of `paid`, which the `paid::derive` call implies.
