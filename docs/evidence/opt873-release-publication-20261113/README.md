# OPT #873 release publication audit

[PR #840](https://github.com/wavect/semaprax/pull/840) merged with base
`6d101efd8c255330a3fe5fb0c06e058c6f8c085f` and head
`4201f657d68f59794693089f451184d655ac8fa0`. Its merge
`ca8d5877c59015fc44ef6ff616057efebbfd2661` has those commits as its two
parents. The head and merge both have tree
`3b275e983ab2ff0f85ab3d446bd2b97d304d456e`. The merge and the
`cc7bb2a5127aeccd9248a7daa30c06ae983f5051` checkpoint are ancestors of
`main` at `f9849aaf2ea5cc2ec01cd7867fa3059f5601b9ed`.

On 2026-11-13, GitHub's pull-request JSON reported 21,177 changed files and
838 commits. The diff media type still returned HTTP 406 with a 300-file
limit. The files endpoint returned 100 rows on page 30 at 100 rows per page
and zero on page 31. The merge establishes publication of the exact head
tree. Reviewability through GitHub remains open in
[#873](https://github.com/wavect/semaprax/issues/873).

| Comparison | Rename-aware files | Paths with rename detection off | Evidence paths with rename detection off |
| --- | ---: | ---: | ---: |
| Original base `bccbe9696` to checkpoint `cc7bb2a51` | 16,035 | 16,045 | 14,999 |
| Final base `6d101efd8` to PR head `4201f657d` | 21,177 | 21,187 | 20,017 |

The final tree delta contains 1,170 changed paths outside `docs/evidence/`.
Evidence packaging alone would therefore leave a delta above the reported
file limit.

## Existing boundaries

The original PR base is an ancestor of the head through the second parent of
`85edab80be892fc691e956cf8460ecc2ded7eb16`. Only three commits lie on
that ancestry path. The transition from the original PR base to
`85edab80b` changes 21,173 paths with rename detection off. Its two
successors add nine and five evidence paths for #882 and #881, respectively.
Those two additions are small review candidates once the preceding tree is
available. An isolated Git index loaded with `85edab80b` reproduced both
successor tree IDs after applying their binary patches in order. The second
application reproduced the exact final release tree.

The feature branch's first-parent chain starts at
`4f8f598c7935d0a5cdd4a5d5ba5b49d164ae2cfc`, an ancestor of the final PR
base. The chain contains 589 transitions, including 108 merges. Comparing
each commit to its first parent with rename detection off found 18
transitions above 300 paths. The largest is `e3eb476838` at 998 paths,
all under `docs/evidence/`. The feature fork and final PR base also differ
by 1,495 paths. Existing first-parent commits give a starting order for
reconstructing the feature. A replay based on the final PR base has to
reconcile the intervening main changes and split oversized transitions while
preserving each consumer's required input bytes.

Git 2.47.1's local three-way merge of the final PR base with the
pre-reconciliation tip `29034d54b` exited 1 and reported 28 conflicted paths,
24 under `docs/evidence/`. Its provisional tree differs from the recorded
reconciliation tree at 30 paths, including two paths without reported
conflicts. The actual reconciliation tree is therefore required as the
comparison target when replaying this boundary.

## Reproduce

```sh
git diff --shortstat 6d101efd8 4201f657d
git diff --name-only --no-renames 6d101efd8 4201f657d
git rev-list --ancestry-path --reverse 6d101efd8..4201f657d
git merge-tree --write-tree --name-only 6d101efd8 29034d54b
git log --first-parent --reverse --format='%H %P' 6d101efd8..4201f657d |
while read -r commit first_parent remaining_parents; do
    printf '%s %s\n' "$commit" "$first_parent"
    git diff --name-only --no-renames "$first_parent" "$commit" | wc -l
done
gh api -H 'Accept: application/vnd.github+json' repos/wavect/semaprax/pulls/840
gh api -H 'Accept: application/vnd.github.v3.diff' repos/wavect/semaprax/pulls/840 --silent
```

A candidate replay still needs an exact final-tree check, the original
assertions and gates, and GitHub diff and mergeability checks for every
proposed partition.
