# Review: `author:find` card search

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

I found one likely concrete defect, one smaller matching defect, and several conditional items. I read the source only and ran nothing.

## Likely defect

**1. `author:routes` probably now exceeds the 2,048-byte `MAX_BYTES`.** (`src/cli/help/authoring.rs`, `ROUTES`)
- I summed the route lines by hand. The header is 29 bytes, the 33 route rows come to about 1,915, and the two old trailer lines are 49 and 42. That is about 2,035 bytes before this change.
- The new `Card search: help language author:find:<word>:0\n` line adds 48 bytes, giving about 2,083.
- My count may be off by a few bytes, but the overshoot is about 35.
- If `author:routes` is held to `MAX_BYTES` (existing tests, or the module's "bounded" contract), this breaks it. Search pages are budgeted exactly, but this card is not.
- Fix: shorten the route summaries, or drop or merge the trailer lines. For example, merge the two search lines, or leave the new search hint to the guide and the `topics` output.
- Please check the exact `ROUTES.len()`.

## Smaller defect

**2. Matching on the synthetic `author:` prefix.** (`discovery.rs`, `selector.contains(word)`)
- `selector` is built as `format!("author:{name}")`, so words such as `au`, `auth`, `thor` and `author` match every card through the prefix alone.
- The docs say search covers "route selector, summary and exact card text". Matching the shared prefix is noise rather than a correct match.
- The unit test `author:find:author:N` depends on this behavior to enumerate all cards.
- Fix: match against `name` instead of `selector`, and use a different enumeration word in the test, or accept the behavior deliberately and document it.

## Conditional items

- **Guide size.** `docs/AGENT-AUTHORING-GUIDE.md` gains a line of about 77 bytes. `assert_guide_contract` requires `len <= 2048` and `lexical_tokens <= 400`. This is only safe if the guide had that much headroom. The guide is also embedded via `include_str!`, so the embedded copy changes with it.
- **String-take card text.** The first line of `author:string-take` changed to `Named String owner transfer (core.string.take); owning qualification pending.` Any existing exact-text or golden assertion on that line will fail.
- **Panics instead of errors.** `search` uses `.expect(...)` on `lookup(&selector)` and on `split_once(' ')`. If a future ROUTES row lacks a card arm, `search` panics instead of returning `Err`. The unit test with word `author` does exercise every row today.

## Checked, no defect found

- **Budget exactness.**
  - Each row is admitted only if `output + row + footer(next+1)` fits.
  - The final footer is `footer(next_final)`, which is the same string that was checked, so the page length is exact, including the changing digit count in `next:`.
  - The complete footer is shorter than the partial one, and that case is accounted for.
- **Frontier and refusal.** `next == start` returns an error naming the exact card. The minimal-budget test is consistent because the header stays 3 digits in the relevant range.
- **Pagination.** Order is by selector, the offset is canonical (`+1`, `00` and `-1` are rejected), and `start >= len` refuses. Continuation offsets are row counts, so they are stable.
- **Malformed selectors.** Missing offset, empty offset, extra segment, empty word, a word over 64 bytes, and non-ASCII or space characters all return `Err`. The word cannot contain `:` because of the split.
- **Recursion.** `lookup` calls `search`, which calls `lookup("author:<name>")`. Those selectors never start with `author:find:`, and the ROUTES search lines do not start with `  author:`, so they are skipped. No cycle.
- **Library `find:` compatibility.** The new branch triggers only on the `author:find:` prefix. `find:*` never reaches it, so library-only `find:` behavior is untouched. The only shared-surface change is the `ROUTES` text noted above.
- **Inert behavior.** The code does no I/O, which fits the empty-working-directory assertion.

## Cannot confirm from the diff

- Whether `author:find:*` errors reach exit code 2 through the existing `language_topic` error path. The new CLI test assumes so.
- Whether `byte-traversal`'s card contains `nested` (the unit test assumes it) and whether `core.string.take` pages stay complete at 2,048 bytes.
