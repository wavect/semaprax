# GitHub Linguist submission v1

Status: interim fix shipped (`.gitattributes` override); upstream submission
not started, gated on real-world adoption this repository cannot manufacture.

Audience: maintainers deciding when to pursue native `Semaprax` recognition on
GitHub, and coding agents asked to advance issue #227.

This page is the plan for GitHub-side syntax highlighting of `.spx` source. It
introduces no new grammar: [editors/vscode/syntaxes/semaprax.tmLanguage.json](../editors/vscode/syntaxes/semaprax.tmLanguage.json)
is the one TextMate grammar this repository owns, already kept in sync with
the parser by `tests/documentation.rs`'s `editor_grammar::grammar_names_every_parser_keyword`
(every parser keyword must appear in the grammar, or that test fails). Any
GitHub-facing submission reuses this file; it does not fork a second grammar
source of truth.

## What is shipped today

`.gitattributes` declares:

```gitattributes
*.spx text eol=lf linguist-language=Rust
```

[`linguist-language` overrides](https://github.com/github-linguist/linguist/blob/main/docs/overrides.md)
are Linguist's own supported mechanism for telling GitHub's hosted renderer
how to classify and highlight a path. This makes `.spx` files render with
Rust's TextMate grammar and count as Rust in `wavect/semaprax`'s language
statistics immediately, with no upstream action, no new repository, and no
grammar work. It is accurate as a rendering hint, not a language claim: `.spx`
is close enough to Rust's surface syntax (`fn`, braces, `->`, block
expressions) that Rust highlighting is a reasonable approximation, but
Semaprax-specific constructs (`permit`, `uses`, `requires`, `ensures`, `@id`)
render as plain identifiers or keywords rather than with dedicated scopes.
This is a deliberate, temporary stand-in, not the destination.

## The destination: native `Semaprax` recognition

Getting GitHub to show **Semaprax** (not Rust, not Text) as the language,
color it distinctly in repository statistics, and highlight it with the
Semaprax-aware grammar requires an accepted entry in
[`github-linguist/linguist`](https://github.com/github-linguist/linguist).
That project's own [contribution requirements](https://github.com/github-linguist/linguist/blob/main/CONTRIBUTING.md)
gate a new programming-language entry on:

- a `languages.yml` entry (extension `.spx`, `tm_scope: source.semaprax`,
  a `type: programming` classification, and a language color);
- the grammar added as a Linguist-managed submodule through
  `script/add-grammar <grammar-repository-url>` — Linguist vendors grammars
  from their **own** dedicated repositories, so the existing
  `editors/vscode/syntaxes/semaprax.tmLanguage.json` would need to be
  published from (or mirrored into) a standalone `semaprax-textmate`-shaped
  repository for that script to consume; it is not added by pointing at a
  path inside `wavect/semaprax`;
- real-world sample files run through `script/update-ids`; and
- **at least 2,000 files using the extension, indexed by GitHub over the last
  year, excluding forks, and reasonably distributed across independent users
  and repositories** — Linguist explicitly guards against a language author
  satisfying this alone from one repository.

## The obstacle, stated plainly

This repository currently has 263 `.spx` files, all inside `wavect/semaprax`
itself (checked at the time of writing; re-run `find . -name '*.spx' | wc -l`
excluding `target/` to refresh the count). That is over an order of magnitude
short of the 2,000-file bar, and every one of those files lives in one
repository from one organization — exactly the concentration Linguist's
independent-distribution requirement is designed to reject. Submitting today
would not be rejected for a missing grammar or a missing `languages.yml`
entry; it would be rejected for insufficient independent real-world usage,
which no amount of work inside this repository can manufacture. Growing
`.spx` usage across independent repositories is a precondition this
submission is gated on, not a task this checklist can close.

## Human-owned checklist (`HUMAN_BLOCKED`)

Everything below requires a decision, external repository creation, or
real-world adoption this repository's own automation has no authority or
ability to produce.

1. **Decide whether and when to keep `linguist-language=Rust`.** It is a
   reasonable interim choice today; revisit it once native recognition is
   pursued, since the two are meant to be mutually exclusive end states (the
   override is removed once GitHub recognizes `Semaprax` natively).
2. **Create and maintain a standalone grammar repository** (for example
   `wavect/semaprax-textmate`) that Linguist's `script/add-grammar` can vendor,
   sourced from (or kept byte-identical to) `editors/vscode/syntaxes/semaprax.tmLanguage.json`
   so the VS Code extension remains the single authored copy. Creating a new
   GitHub repository is outside this repository's own change scope.
3. **Grow independent `.spx` adoption past Linguist's 2,000-file,
   multi-repository bar.** This is an ecosystem outcome, not an engineering
   task; there is no shortcut inside `wavect/semaprax`.
4. **Open the `github-linguist/linguist` pull request** once the above hold:
   add the `Semaprax` entry to `languages.yml`, run `script/add-grammar`
   against the standalone grammar repository, add real-world `.spx` samples,
   run `script/update-ids`, and submit with the adoption evidence Linguist's
   maintainers ask for. `HUMAN_BLOCKED: needs an upstream github/linguist PR`
   — this repository's automation cannot open or merge a pull request against
   a repository it does not own.
5. **Only after that PR is accepted**, remove `linguist-language=Rust` from
   `.gitattributes` here and update this page's "What is shipped today"
   section to describe native recognition instead of the interim override.
