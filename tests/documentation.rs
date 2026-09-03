use std::path::{Path, PathBuf};

#[test]
fn local_markdown_links_resolve() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut documents = [
        "AGENTS.md",
        "CLAUDE.md",
        "CHANGELOG.md",
        "CONTRIBUTING.md",
        "README.md",
        "SECURITY.md",
    ]
    .into_iter()
    .map(|path| root.join(path))
    .collect::<Vec<_>>();
    collect_markdown(&root.join("docs"), &mut documents);
    documents.sort();

    for document in documents {
        let source = std::fs::read_to_string(&document).unwrap();
        for target in markdown_targets(&source) {
            if target.starts_with("https://")
                || target.starts_with("http://")
                || target.starts_with('#')
                || target.starts_with("mailto:")
            {
                continue;
            }
            let target = target.split('#').next().unwrap();
            if target.is_empty() {
                continue;
            }
            let resolved = document.parent().unwrap().join(target);
            assert!(
                resolved.exists(),
                "{} links to missing local target {target}",
                document.display()
            );
        }
    }
}

#[test]
fn documentation_catalog_and_metadata_are_complete() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let docs = root.join("docs");
    let summary = std::fs::read_to_string(docs.join("SUMMARY.md")).unwrap();
    let mut documents = Vec::new();
    collect_markdown(&docs, &mut documents);
    documents.sort();

    for document in documents {
        let source = std::fs::read_to_string(&document).unwrap();
        let mut lines = source.lines();
        let title = lines.next().unwrap_or_default();
        assert!(
            title.starts_with("# "),
            "{} must start with one H1 title",
            document.display()
        );

        let metadata = source.lines().take(12).collect::<Vec<_>>();
        assert!(
            metadata.iter().any(|line| line
                .strip_prefix("- ")
                .unwrap_or(line)
                .starts_with("Status:")),
            "{} must state its status within the first 12 lines",
            document.display()
        );
        assert!(
            metadata.iter().any(|line| line
                .strip_prefix("- ")
                .unwrap_or(line)
                .starts_with("Audience:")),
            "{} must state its audience within the first 12 lines",
            document.display()
        );

        if document
            .file_name()
            .is_some_and(|name| name == "SUMMARY.md")
        {
            continue;
        }
        let relative = document.strip_prefix(&docs).unwrap();
        let catalog_entry = format!("]({})", relative.to_string_lossy().replace('\\', "/"));
        assert!(
            summary.contains(&catalog_entry),
            "{} is missing from docs/SUMMARY.md",
            document.display()
        );
    }
}

mod language_tour {
    use std::path::{Path, PathBuf};

    const TOUR: &str = "docs/LANGUAGE-TOUR.md";
    const FENCE: &str = "```semaprax";
    const CITATION_PREFIX: &str = "../examples/";

    #[test]
    fn every_tour_code_block_is_a_verbatim_example_excerpt() {
        let tour = tour_path();
        let source = read_normalized(&tour);
        let lines = source.lines().collect::<Vec<_>>();
        let mut blocks = 0usize;
        let mut index = 0usize;

        while index < lines.len() {
            if lines[index].trim() != FENCE {
                index += 1;
                continue;
            }

            let mut cursor = index;
            while cursor > 0 && lines[cursor - 1].trim().is_empty() {
                cursor -= 1;
            }
            assert!(
                cursor > 0,
                "{TOUR} line {}: a {FENCE} block must be preceded by a line citing its example",
                index + 1
            );
            let citation = lines[cursor - 1];
            let Some(relative) = example_link(citation) else {
                panic!(
                    "{TOUR} line {}: the line before a {FENCE} block must link exactly one \
                     {CITATION_PREFIX}<name>.spx file, found {citation:?}",
                    index + 1
                );
            };
            let example = tour.parent().unwrap().join(relative);
            assert!(
                example.exists(),
                "{TOUR} line {} cites missing example {relative}",
                index + 1
            );

            let start = index + 1;
            let mut end = start;
            while end < lines.len() && lines[end].trim_end() != "```" {
                end += 1;
            }
            assert!(
                end < lines.len(),
                "{TOUR} line {}: unterminated {FENCE} block",
                index + 1
            );

            let block = lines[start..end].join("\n");
            assert!(
                !block.trim().is_empty(),
                "{TOUR} line {}: empty {FENCE} block",
                index + 1
            );
            let example_source = read_normalized(&example);
            assert!(
                example_source.contains(&block),
                "{TOUR} lines {}-{} are not a verbatim excerpt of {relative}; \
                 update the tour or the example so the block matches byte for byte",
                start + 1,
                end
            );

            blocks += 1;
            index = end + 1;
        }

        assert!(
            blocks >= 12,
            "{TOUR} must keep at least twelve verified SEMAPRAX excerpts, found {blocks}"
        );
    }

    #[test]
    fn every_example_the_tour_cites_exists() {
        let tour = tour_path();
        let source = read_normalized(&tour);
        let mut cited = 0usize;

        for target in super::markdown_targets(&source) {
            if !target.starts_with(CITATION_PREFIX) {
                continue;
            }
            let resolved = tour.parent().unwrap().join(target);
            assert!(
                resolved.exists(),
                "{TOUR} cites missing example path {target}"
            );
            cited += 1;
        }

        assert!(
            cited >= 12,
            "{TOUR} must cite its examples by link, found {cited}"
        );
    }

    fn tour_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(TOUR)
    }

    fn read_normalized(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap().replace("\r\n", "\n")
    }

    fn example_link(line: &str) -> Option<&str> {
        let mut links = super::markdown_targets(line)
            .into_iter()
            .filter(|target| target.starts_with(CITATION_PREFIX) && target.ends_with(".spx"));
        let first = links.next()?;
        if links.next().is_some() {
            return None;
        }
        Some(first)
    }
}

fn collect_markdown(directory: &Path, output: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_markdown(&path, output);
        } else if path.extension().is_some_and(|extension| extension == "md") {
            output.push(path);
        }
    }
}

fn markdown_targets(source: &str) -> Vec<&str> {
    let mut targets = Vec::new();
    let mut remaining = source;
    while let Some(start) = remaining.find("](") {
        remaining = &remaining[start + 2..];
        let Some(end) = remaining.find(')') else {
            break;
        };
        targets.push(remaining[..end].trim_matches(['<', '>']));
        remaining = &remaining[end + 1..];
    }
    targets
}

mod agent_quick_reference {
    //! Every `semaprax` block in the agent quick reference is a complete module
    //! the compiler agrees with. An unmarked block verifies without diagnostics
    //! and is already canonical, so an agent that copies it never pays for a
    //! `fmt` or `check` round trip. A block preceded by
    //! `<!-- expect: SPX-XNNN -->` must produce exactly that diagnostic code, so
    //! the page's "what the compiler says" claims cannot drift from the parser
    //! and verifier.

    use std::path::Path;

    use semaprax::{format, parse, verify};

    const REFERENCE: &str = "docs/AGENT-QUICK-REFERENCE.md";
    const FENCE: &str = "```semaprax";
    const MARKER_PREFIX: &str = "<!-- expect: ";
    const MARKER_SUFFIX: &str = " -->";

    struct Block {
        line: usize,
        source: String,
        expect: Option<String>,
    }

    fn blocks() -> Vec<Block> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(REFERENCE);
        let text = std::fs::read_to_string(&path)
            .unwrap()
            .replace("\r\n", "\n");
        let lines = text.lines().collect::<Vec<_>>();
        let mut blocks = Vec::new();
        let mut index = 0usize;
        while index < lines.len() {
            if lines[index].trim() != FENCE {
                index += 1;
                continue;
            }
            let mut cursor = index;
            while cursor > 0 && lines[cursor - 1].trim().is_empty() {
                cursor -= 1;
            }
            let expect = cursor
                .checked_sub(1)
                .map(|previous| lines[previous].trim())
                .and_then(|line| {
                    line.strip_prefix(MARKER_PREFIX)?
                        .strip_suffix(MARKER_SUFFIX)
                        .map(str::to_owned)
                });
            let start = index + 1;
            let mut end = start;
            while end < lines.len() && lines[end].trim_end() != "```" {
                end += 1;
            }
            assert!(
                end < lines.len(),
                "{REFERENCE} line {}: unterminated {FENCE} block",
                index + 1
            );
            let mut source = lines[start..end].join("\n");
            source.push('\n');
            blocks.push(Block {
                line: index + 1,
                source,
                expect,
            });
            index = end + 1;
        }
        blocks
    }

    #[test]
    fn unmarked_blocks_verify_cleanly_and_are_canonical() {
        let mut checked = 0usize;
        for block in blocks().into_iter().filter(|block| block.expect.is_none()) {
            let path = format!("agent-quick-reference-{}.spx", block.line);
            let program = parse(&block.source, Path::new(&path)).unwrap_or_else(|diagnostic| {
                panic!(
                    "{REFERENCE} line {}: block does not parse: {diagnostic}",
                    block.line
                )
            });
            let diagnostics = verify::verify(&program);
            assert!(
                diagnostics.is_empty(),
                "{REFERENCE} line {}: block must verify without diagnostics, found {:?}",
                block.line,
                diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.to_string())
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                format::canonical(&program),
                block.source,
                "{REFERENCE} line {}: block is not canonical; paste the formatter output",
                block.line
            );
            checked += 1;
        }
        assert!(
            checked >= 5,
            "{REFERENCE} must keep at least five verified clean modules, found {checked}"
        );
    }

    #[test]
    fn marked_blocks_produce_exactly_the_named_diagnostic() {
        let mut checked = 0usize;
        for block in blocks() {
            let Some(expected) = block.expect.as_deref() else {
                continue;
            };
            let path = format!("agent-quick-reference-{}.spx", block.line);
            let codes = match parse(&block.source, Path::new(&path)) {
                Err(diagnostic) => vec![diagnostic.code.to_owned()],
                Ok(program) => verify::verify(&program)
                    .into_iter()
                    .filter(|diagnostic| diagnostic.severity.is_error())
                    .map(|diagnostic| diagnostic.code.to_owned())
                    .collect(),
            };
            assert!(
                !codes.is_empty(),
                "{REFERENCE} line {}: block is marked `expect: {expected}` but verifies",
                block.line
            );
            assert!(
                codes.iter().all(|code| code == expected),
                "{REFERENCE} line {}: block is marked `expect: {expected}` but produced {codes:?}; \
                 a habit example must demonstrate one diagnostic",
                block.line
            );
            checked += 1;
        }
        assert!(
            checked >= 5,
            "{REFERENCE} must keep at least five diagnostic demonstrations, found {checked}"
        );
    }
}
