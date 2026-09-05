//! A deterministic, scalable multi-module Project fixture.
//!
//! The benchmarks measure real Project operations, so they need a subject whose
//! size can be varied without hand-writing modules, and whose edits are
//! controlled: one leaf changed (few consumers) versus the shared provider
//! changed (every consumer). The same generator backs the smoke test that keeps
//! the fixture admissible, so a benchmark can never quietly measure a project
//! the compiler would reject.
//!
//! Nothing here bypasses admission: the fixture is ordinary `.spx` source and
//! an ordinary `semaprax.project.v1` manifest.

// Each including target uses a subset: the interpreter benchmark needs only
// the scalar-loop shapes, the project benchmark and the smoke test need the
// scaled multi-module fixture.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// Fixture sizes benchmarked together: 1x, 2x and 4x the base module count.
pub const SCALES: [usize; 3] = [1, 2, 4];

/// Leaf modules per scale unit. A Project v1 manifest admits at most sixteen
/// sources, so the 4x fixture must stay inside that bound.
const LEAVES_PER_SCALE: usize = 2;

/// Functions declared by every leaf module.
const FUNCTIONS_PER_LEAF: usize = 3;

pub struct ProjectFixture {
    pub scale: usize,
    pub manifest: String,
    /// Manifest-relative source paths with their exact bytes, in manifest order.
    pub sources: Vec<(String, String)>,
}

impl ProjectFixture {
    pub fn leaves(&self) -> usize {
        self.scale * LEAVES_PER_SCALE
    }

    /// Source bytes the frontend actually reads: the work unit these benches
    /// report as throughput.
    pub fn source_bytes(&self) -> u64 {
        self.sources
            .iter()
            .map(|(_, source)| source.len() as u64)
            .sum()
    }

    /// Declared functions across the fixture, including the entry and test
    /// modules. Reported alongside the byte count so a size change is visible.
    pub fn declarations(&self) -> usize {
        1 + self.leaves() * FUNCTIONS_PER_LEAF + 2
    }

    /// The same fixture with exactly one leaf module changed. Only that leaf
    /// and its consumers may be reanalysed.
    pub fn with_edited_leaf(&self) -> Vec<(String, String)> {
        self.edit("src/leaf_0.spx", "scale(value) + 0", "0 + scale(value)")
    }

    /// The same fixture with the shared provider changed. Every leaf, the entry
    /// module and the test module consume it transitively.
    pub fn with_edited_core(&self) -> Vec<(String, String)> {
        self.edit("src/core.spx", "value * 3 + 1", "1 + value * 3")
    }

    /// A byte-level edit that keeps the module canonical, admissible and
    /// result-identical: one commuted addition. The benchmark measures
    /// reanalysis, not a different program, and a Project source that stopped
    /// being canonical would be rejected rather than measured.
    fn edit(&self, path: &str, from: &str, to: &str) -> Vec<(String, String)> {
        let mut edited = self.sources.clone();
        let entry = edited
            .iter_mut()
            .find(|(name, _)| name == path)
            .unwrap_or_else(|| panic!("fixture has no source {path}"));
        assert!(
            entry.1.contains(from),
            "{path} does not contain the edited expression {from:?}"
        );
        entry.1 = canonical(path, &entry.1.replacen(from, to, 1));
        assert_ne!(
            entry.1,
            self.sources
                .iter()
                .find(|(name, _)| name == path)
                .unwrap()
                .1
        );
        edited
    }

    /// Write the fixture into a directory and return its manifest path.
    ///
    /// The directory is canonicalised: Project loading rejects a symlinked
    /// ancestor, and the platform temporary directory is often one.
    pub fn write_to(&self, directory: &Path) -> PathBuf {
        let directory = &std::fs::canonicalize(directory).unwrap();
        std::fs::create_dir_all(directory.join("src")).unwrap();
        for (path, source) in &self.sources {
            std::fs::write(directory.join(path), source).unwrap();
        }
        let manifest = directory.join("semaprax.toml");
        std::fs::write(&manifest, &self.manifest).unwrap();
        manifest
    }
}

/// Build the fixture for one scale factor. Deterministic: the same scale always
/// produces the same bytes.
pub fn generate(scale: usize) -> ProjectFixture {
    assert!(scale > 0, "fixture scale must be positive");
    let leaves = scale * LEAVES_PER_SCALE;
    let mut sources = vec![
        ("src/app.spx".to_owned(), entry_module(leaves)),
        ("src/core.spx".to_owned(), core_module()),
    ];
    for leaf in 0..leaves {
        sources.push((format!("src/leaf_{leaf}.spx"), leaf_module(leaf)));
    }
    sources.push(("src/tests.spx".to_owned(), test_module(leaves)));
    // A Project v1 manifest carries byte-sorted source paths, and the manifest
    // text must equal its canonical rendering exactly.
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    for source in &mut sources {
        source.1 = canonical(&source.0, &source.1);
    }

    let manifest = format!(
        "schema = \"semaprax.project.v1\"\n\
         name = \"bench-scale-{scale}\"\n\
         entry = \"bench.app\"\n\
         sources = {}\n\
         web_exports = [\"bench.core.scale\"]\n\
         tests = [\"bench.tests\"]\n",
        render_array(&sources)
    );
    ProjectFixture {
        scale,
        manifest,
        sources,
    }
}

/// Canonical source bytes for one generated module.
///
/// A Project source must already be canonical, so the generator renders its
/// modules through the compiler's own formatter rather than guessing the
/// layout. This is the ordinary formatter, not an admission bypass.
fn canonical(path: &str, source: &str) -> String {
    let program = semaprax::parse(source, Path::new(path))
        .unwrap_or_else(|error| panic!("generated {path} must parse: {error:?}"));
    semaprax::format::canonical(&program)
}

/// Canonical Project manifest array rendering: `["a", "b"]`.
fn render_array(sources: &[(String, String)]) -> String {
    let inventory = sources
        .iter()
        .map(|(path, _)| format!("\"{path}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{inventory}]")
}

fn core_module() -> String {
    "module bench.core;\n\
     \n\
     @id(\"bench.core.scale\")\n\
     fn scale(value: i64) -> i64\n\
     {\n    \
         value * 3 + 1\n\
     }\n"
    .to_owned()
}

fn leaf_module(leaf: usize) -> String {
    let mut source = format!(
        "module bench.leaf_{leaf};\n\
         use function @id(\"bench.core.scale\") from bench.core as scale;\n\n"
    );
    for function in 0..FUNCTIONS_PER_LEAF {
        source.push_str(&format!(
            "@id(\"bench.leaf_{leaf}.step_{function}\")\n\
             fn step_{function}(value: i64) -> i64\n\
             {{\n    \
                 scale(value) + {function}\n\
             }}\n\n"
        ));
    }
    source
}

fn entry_module(leaves: usize) -> String {
    let mut source = "module bench.app;\n".to_owned();
    for leaf in 0..leaves {
        source.push_str(&format!(
            "use function @id(\"bench.leaf_{leaf}.step_0\") from bench.leaf_{leaf} as leaf_{leaf};\n"
        ));
    }
    source.push_str("\n@id(\"bench.app.main\")\nfn main() -> i64\n{\n    ");
    let sum = (0..leaves)
        .map(|leaf| format!("leaf_{leaf}({leaf})"))
        .collect::<Vec<_>>()
        .join(" + ");
    source.push_str(&sum);
    source.push_str("\n}\n");
    source
}

fn test_module(leaves: usize) -> String {
    let mut source = "module bench.tests;\n".to_owned();
    for leaf in 0..leaves {
        source.push_str(&format!(
            "use function @id(\"bench.leaf_{leaf}.step_1\") from bench.leaf_{leaf} as leaf_{leaf};\n"
        ));
    }
    source.push_str("\n@id(\"bench.tests.main\")\nfn main() -> i64\n{\n    if ");
    // `scale(0)` is 1 and `step_1` adds 1, so every leaf returns 2 for input 0.
    let checks = (0..leaves)
        .map(|leaf| format!("leaf_{leaf}(0) == 2"))
        .collect::<Vec<_>>()
        .join(" && ");
    source.push_str(&checks);
    source.push_str(" { 0 } else { 1 }\n}\n");
    source
}

/// A single-file scalar loop module: the smallest subject that separates
/// evaluator cost from reading, parsing and verifying a file.
pub fn scalar_loop_source(iterations: u64) -> String {
    format!(
        "module bench.scalar;\n\
         \n\
         @id(\"bench.scalar.step\")\n\
         fn step(value: i64) -> i64\n\
         {{\n    \
             value + 1\n\
         }}\n\
         \n\
         @id(\"bench.scalar.main\")\n\
         fn main() -> i64\n\
         {{\n    \
             let mut acc = 0;\n    \
             let mut i = 0;\n    \
             while i < {iterations} {{\n        \
                 acc = (acc + i * 3) % 1000003;\n        \
                 i = i + 1;\n        \
                 i < {iterations}\n    \
             }}\n    \
             acc\n\
         }}\n"
    )
}

/// The same scalar loop as a Project, so a prepared evaluator can execute it
/// repeatedly without re-reading or re-verifying any source. A Project v1
/// manifest declares at least two sources and exactly one test module.
pub fn scalar_loop_project(iterations: u64) -> ProjectFixture {
    let tests = "module bench.scalar_tests;\n\
                 \n\
                 @id(\"bench.scalar_tests.main\")\n\
                 fn main() -> i64\n\
                 {\n    \
                     0\n\
                 }\n"
    .to_owned();
    ProjectFixture {
        scale: 1,
        manifest: "schema = \"semaprax.project.v1\"\n\
                   name = \"bench-scalar\"\n\
                   entry = \"bench.scalar\"\n\
                   sources = [\"src/loop.spx\", \"src/tests.spx\"]\n\
                   web_exports = [\"bench.scalar.step\"]\n\
                   tests = [\"bench.scalar_tests\"]\n"
            .to_owned(),
        sources: vec![
            (
                "src/loop.spx".to_owned(),
                canonical("src/loop.spx", &scalar_loop_source(iterations)),
            ),
            (
                "src/tests.spx".to_owned(),
                canonical("src/tests.spx", &tests),
            ),
        ],
    }
}
