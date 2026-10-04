use semaprax::project::with_authenticated_project;
use semaprax_native_rust_interop::prepare_native_rust_serde_iterator_callbacks;
use std::{fs, path::Path};

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = root.join("project/semaprax.toml");
    let source_path = root.join("project/app.spx");
    let projection = with_authenticated_project(&manifest, |snapshot| {
        snapshot.check()?;
        let source = snapshot
            .sources()
            .iter()
            .find(|source| source.path() == "app.spx")
            .expect("authenticated M2 manifest requires app.spx");
        prepare_native_rust_serde_iterator_callbacks(
            source.source(),
            &source_path,
            "ri13.event",
            "callback.factory",
            "callback.advance",
        )
    })
    .expect("one authenticated M2 Project source revision with a record and iterator callback");
    let generated = root.join("generated");
    fs::create_dir_all(&generated).unwrap();
    fs::write(generated.join("module.c"), &projection.callback.c_source).unwrap();
    fs::write(
        generated.join("semaprax_native_rust_interop.h"),
        &projection.callback.header,
    )
    .unwrap();
    fs::write(
        root.join("src/semaprax_native_rust_interop_ffi.rs"),
        &projection.callback.ffi_rust,
    )
    .unwrap();
    fs::write(
        root.join("src/generated.rs"),
        format!(
            "{}\n{}\n{}\n",
            projection.callback.safe_rust,
            projection.callback.adapter_rust,
            projection.record.rust_source
        ),
    )
    .unwrap();
    println!("ri13-m2-prepared:{}", projection.source_revision);
}
