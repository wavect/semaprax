use semaprax::project::with_authenticated_project;
use std::path::Path;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = root.join("project/semaprax.toml");
    let generated = with_authenticated_project(&manifest, |snapshot| {
        snapshot.check()?;
        snapshot.render_source_local_future_rust_module()
    })
    .expect("admitted held Project");
    let output = root.join("src/generated.rs");
    if output.is_file() && std::fs::read(&output).unwrap() == generated.as_bytes() {
        println!("ri13-m3-prepared");
        return;
    }
    let staged = root.join(format!("src/.generated-{}.tmp", std::process::id()));
    std::fs::write(&staged, generated).unwrap();
    std::fs::rename(&staged, &output).unwrap();
    println!("ri13-m3-prepared");
}
