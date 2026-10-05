//! TEMPORARY, UNCOMMITTED: exact-output baseline dump for REF-09/10/11.
use semaprax::graph::{
    self, AgentContextDirection, AgentContextFilter, AgentContextOptions, AgentContextV2Options,
};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn render<T: std::fmt::Debug>(result: Result<Option<String>, T>) -> String {
    match result {
        Ok(Some(text)) => format!("ok\t{text}"),
        Ok(None) => "none".to_owned(),
        Err(error) => format!("err\t{error:?}"),
    }
}

fn spx_files(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let mut entries = entries.flatten().map(|e| e.path()).collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if name == "target" || name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            spx_files(&path, out);
        } else if name.ends_with(".spx") {
            out.push(path);
        }
    }
}

fn synthetic() -> Vec<(String, String)> {
    let mut wide = String::from("module test.ref_wide;\n@id(\"app.main\") fn main() -> i64 { root(1) }\n@id(\"w.root\") fn root(v: i64) -> i64 { 0");
    for i in 0..40 {
        write!(wide, " + f{i}(v)").unwrap();
    }
    wide.push_str(" }\n");
    for i in 0..40 {
        let name_len = 3 + (i * 7) % 60;
        let pad = "x".repeat(name_len);
        writeln!(wide, "@id(\"w.f{i}.{pad}\") fn f{i}(v: i64) -> i64 requires v >= {i} {{ if v > 0 {{ f{}(v - 1) }} else {{ {i} }} }}", (i + 1) % 40).unwrap();
    }
    for i in 0..20 {
        writeln!(wide, "@id(\"w.unrelated{i}\") fn unrelated{i}() -> i64 {{ {i} }}").unwrap();
    }
    let strings = "module test.ref_strings;\n@id(\"app.main\") fn main() -> i64 { s.a(1) }\n@id(\"s.a\") fn a(v: i64) -> i64 { let t = \"quote \\\" back \\\\ caf\u{e9} \u{1F600}\"; b(v) + c(v) }\n@id(\"s.b\") fn b(v: i64) -> i64 { let u = \"tab\\t nl\\n \u{4E2D}\u{6587}\"; a(v - 1) }\n@id(\"s.c\") fn c(v: i64) -> i64 { v }\n".replace("s.a(1)", "a(1)");
    let mut cyclic = String::from("module test.ref_cyclic;\n@id(\"app.main\") fn main() -> i64 { c0(1) }\n");
    for i in 0..12 {
        writeln!(cyclic, "@id(\"c.n{i}\") fn c{i}(v: i64) -> i64 {{ if v > 0 {{ c{}(v - 1) + c{}(v - 2) }} else {{ 0 }} }}", (i + 1) % 12, (i + 5) % 12).unwrap();
    }
    vec![
        ("synthetic/wide.spx".to_owned(), wide),
        ("synthetic/strings.spx".to_owned(), strings),
        ("synthetic/cyclic.spx".to_owned(), cyclic),
    ]
}

#[test]
fn dump() {
    let Ok(out_dir) = std::env::var("REF_DUMP_DIR") else {
        return;
    };
    let out_dir = PathBuf::from(out_dir);
    std::fs::create_dir_all(&out_dir).unwrap();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for dir in ["examples", "tests", "std", "src", "platform-tests", "crates", "benchmarks"] {
        spx_files(&repo.join(dir), &mut files);
    }
    let mut sources = synthetic();
    for path in files {
        let rel = path.strip_prefix(repo).unwrap().to_string_lossy().to_string();
        if let Ok(text) = std::fs::read_to_string(&path) {
            sources.push((rel, text));
        }
    }
    let all = AgentContextFilter::ALL;
    let core = [
        AgentContextFilter::Contracts,
        AgentContextFilter::Ownership,
        AgentContextFilter::Effects,
        AgentContextFilter::Types,
    ];
    let mut parsed = 0;
    for (rel, text) in sources {
        let Ok(program) = semaprax::parse(&text, Path::new(&rel)) else {
            continue;
        };
        parsed += 1;
        let synthetic = rel.starts_with("synthetic/");
        let bench = rel.starts_with("benchmarks/");
        let mut dump = String::new();
        writeln!(dump, "graph\t{}", render(graph::to_json(&program).map(Some))).unwrap();
        let mut roots = Vec::new();
        if let Ok(resolved) = semaprax::hir::resolve(&program) {
            roots.extend(resolved.functions.iter().map(|f| f.id.as_str().to_owned()));
            roots.extend(
                resolved
                    .function_templates
                    .iter()
                    .map(|t| t.id.as_str().to_owned()),
            );
        }
        roots.push("no.such.root".to_owned());
        if bench {
            roots.truncate(3);
        }
        let mut grid: Vec<(usize, usize, usize, bool)> = vec![
            (1, 64 * 1024, 256, true),
            (8, 4096, 256, true),
            (8, 2048, 256, false),
            (2, 64 * 1024, 1, true),
        ];
        if synthetic {
            for budget in (2048..24_000).step_by(97) {
                grid.push((8, budget, 256, true));
            }
            grid.push((3, 16 * 1024 * 1024, 65_536, true));
        }
        for root in &roots {
            for &(depth, bytes, nodes, full) in &grid {
                let filters: &[AgentContextFilter] = if full { &core } else { &[AgentContextFilter::Effects] };
                let v1 = AgentContextOptions::new(depth, bytes, nodes, filters.iter().copied())
                    .unwrap();
                writeln!(
                    dump,
                    "v1\t{root}\t{depth}\t{bytes}\t{nodes}\t{full}\t{}",
                    render(graph::agent_context_json(&program, root, &v1))
                )
                .unwrap();
                for direction in AgentContextDirection::ALL {
                    if !synthetic && nodes == 1 {
                        continue;
                    }
                    let v2 = AgentContextV2Options::new(
                        depth,
                        bytes,
                        nodes,
                        filters.iter().copied(),
                        direction,
                    )
                    .unwrap();
                    writeln!(
                        dump,
                        "v2\t{root}\t{direction:?}\t{depth}\t{bytes}\t{nodes}\t{full}\t{}",
                        render(graph::agent_context_v2_json(&program, root, &v2))
                    )
                    .unwrap();
                }
            }
            if !bench {
                let v1 = AgentContextOptions::new(1, 64 * 1024, 256, all).unwrap();
                writeln!(
                    dump,
                    "v1all\t{root}\t{}",
                    render(graph::agent_context_json(&program, root, &v1))
                )
                .unwrap();
            }
        }
        let name = rel.replace('/', "__");
        std::fs::write(out_dir.join(format!("{name}.txt")), dump).unwrap();
    }
    eprintln!("parsed {parsed} programs");
}
