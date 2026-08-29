pub(super) const TEMPLATE_NAME: &str = "calculator";
pub(super) const INVENTORY: [&str; 4] =
    ["README.md", "semaprax.toml", "src/app.spx", "src/tests.spx"];

const README: &str = "# {{name}}\n\nA small calculator project created by SEMAPRAX.\n\n```sh\nsemaprax check semaprax.toml\nsemaprax test semaprax.toml\nsemaprax run semaprax.toml\nsemaprax build semaprax.toml --target web -o web\n```\n";

const MANIFEST: &str = "schema = \"semaprax.project.v1\"\nname = \"{{name}}\"\nentry = \"{{module}}.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"{{name}}.add\"]\ntests = [\"{{module}}.tests\"]\n";

const APP: &str = "module {{module}}.app;\n\n@id(\"{{name}}.add\")\nfn add(left: i64, right: i64) -> i64\n{\n    left + right\n}\n\n@id(\"{{name}}.app.main\")\nfn main() -> i64\n{\n    add(19, 23)\n}\n";

const TESTS: &str = "module {{module}}.tests;\n\n@id(\"{{name}}.tests.main\")\nfn main() -> i64\n{\n    if 19 + 23 == 42 { 0 } else { 1 }\n}\n";

pub(super) struct TemplateFile {
    pub(super) path: &'static str,
    pub(super) bytes: Vec<u8>,
}

pub(super) fn render(name: &str) -> Vec<TemplateFile> {
    let module = name.replace('-', "_");
    [
        ("README.md", README),
        ("semaprax.toml", MANIFEST),
        ("src/app.spx", APP),
        ("src/tests.spx", TESTS),
    ]
    .into_iter()
    .map(|(path, source)| TemplateFile {
        path,
        bytes: source
            .replace("{{name}}", name)
            .replace("{{module}}", &module)
            .into_bytes(),
    })
    .collect()
}
