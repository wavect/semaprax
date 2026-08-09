//! Emit the exact private desktop provider source and descriptor.

use std::error::Error;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use semaprax::codegen::{emit_private_native_callable_v3_fixture, PrivateNativeCallableV3Fixture};
use semaprax::hir::DeclarationId;
use semaprax::owned_resource_corpus::build_owned_resource_corpus_v1;

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args_os();
    let _program = arguments.next();
    let source = PathBuf::from(arguments.next().ok_or_else(usage)?);
    let descriptor = PathBuf::from(arguments.next().ok_or_else(usage)?);
    if arguments.next().is_some()
        || !source.is_absolute()
        || !descriptor.is_absolute()
        || source == descriptor
    {
        return Err(usage().into());
    }
    let corpus = build_owned_resource_corpus_v1()
        .map_err(|error| io::Error::other(format!("build corpus: {error:?}")))?;
    let artifact = emit_private_native_callable_v3_fixture(
        &corpus.program,
        &DeclarationId::new("token.identity"),
        PrivateNativeCallableV3Fixture::OwnedIdentity,
    )
    .map_err(|error| io::Error::other(format!("emit provider: {error:?}")))?;
    let translation_unit = format!(
        "{}\nstatic void spx_v3_generated_finalize(uint32_t owner,uint64_t payload){{(void)owner;(void)payload;}}\n",
        artifact.source()
    );
    write_new(&source, translation_unit.as_bytes())?;
    write_new(&descriptor, artifact.descriptor())?;
    Ok(())
}

fn usage() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "expected two distinct absolute output paths: provider.c descriptor.spxnabi3",
    )
}

fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.flush()
}
