# TeamDesk change requests

Three maintenance tasks applied, in order, to a complete implementation of
[SPEC.md](SPEC.md). Each is measured separately: the tokens of the files an
agent must read to make the change, and the tokens of the edits it writes.

1. **Customer website.** Add `website: string` to Customer. Rule: website
   starts with `https://`. It appears in every list, detail, and form like
   any other field.
2. **Critical priority.** Add a fifth Priority value `Critical` after Urgent.
   Task `weight` multiplies estimate by 8 for it.
3. **Tags.** Add a new entity Tag (name: string of 1..30 bytes) with the full
   API and UI of every other entity, and a reference `tag_id` on Task.
