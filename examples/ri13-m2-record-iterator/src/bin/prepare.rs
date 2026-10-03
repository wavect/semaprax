use semaprax_native_rust_interop::prepare_native_rust_serde_iterator_callbacks;
use std::{fs, path::Path};

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_path = root.join("project/app.spx");
    let source = fs::read_to_string(&source_path).expect("saved source");
    let projection = prepare_native_rust_serde_iterator_callbacks(
        &source,
        &source_path,
        "ri13.event",
        "callback.factory",
        "callback.advance",
    )
    .expect("one checked source revision with a record and iterator callback");
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
