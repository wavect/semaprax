//! Separate safe Native Rust owned-data SDK package.

use super::authentication::{authenticate_inventory, hold_matching};
use super::owned_data_descriptor::{replay_descriptor, verify_manifest, ManifestFacts};
use super::*;

pub(super) const OWNED_CRATE_NAME: &str = "semaprax-generated-native-rust-owned-data-sdk";
pub(super) const OWNED_CRATE_VERSION: &str = "0.1.0";
const OWNED_MANIFEST_DOMAIN: &[u8] = b"semaprax.native-rust-owned-data-sdk.manifest.v1\0";
const MAX_PROVIDER_BYTES: usize = 8 * 1024 * 1024;
const MAX_ARCHIVE_BYTES: usize = 8 * 1024 * 1024;

pub fn build_native_rust_owned_data_sdk(
    program: &semaprax::hir::ResolvedProgram,
    selected: &[String],
    subject: semaprax::project::PublicApiSubject<'_>,
    descriptor_bytes: &[u8],
    descriptor_digest: &str,
    output: &Path,
) -> Result<NativeRustOwnedDataSdkBundle, Vec<Diagnostic>> {
    build(
        program,
        selected,
        subject,
        descriptor_bytes,
        descriptor_digest,
        output,
    )
    .map_err(|error| vec![error])
}

fn build(
    program: &semaprax::hir::ResolvedProgram,
    selected: &[String],
    subject: semaprax::project::PublicApiSubject<'_>,
    descriptor_bytes: &[u8],
    descriptor_digest: &str,
    output: &Path,
) -> Result<NativeRustOwnedDataSdkBundle, Diagnostic> {
    let descriptor = semaprax::project::replay_public_api_descriptor(
        program,
        selected,
        subject,
        descriptor_bytes,
        descriptor_digest,
    )?;
    replay_descriptor(descriptor_bytes, &descriptor)?;
    let provider = semaprax::codegen::emit_native_owned_data_provider(
        program,
        selected,
        subject,
        descriptor_bytes,
        descriptor_digest,
    )?;
    if provider.source().len() > MAX_PROVIDER_BYTES
        || provider.descriptor() != descriptor_bytes
        || provider.descriptor_digest() != descriptor_digest
    {
        return Err(error("owned-data provider artifact authentication failed"));
    }
    let target = target_triple().ok_or_else(|| error("owned-data SDK target is unsupported"))?;
    let (cargo_toml, build_rs, lib_rs, ffi_rs) = render_sources(&descriptor, target);
    let scratch = output.parent().ok_or_else(publication_error)?;
    let archive = build_archive(provider.source().as_bytes(), target, scratch)?;
    let archive_name = if cfg!(windows) {
        "semaprax_native_rust_owned_data_sdk.lib"
    } else {
        "libsemaprax_native_rust_owned_data_sdk.a"
    };
    let manifest = render_manifest(
        target,
        descriptor_bytes,
        descriptor_digest,
        archive_name,
        [
            ("Cargo.toml", cargo_toml.as_bytes()),
            ("build.rs", build_rs.as_bytes()),
            ("lib.rs", lib_rs.as_bytes()),
            ("owned_data_ffi.rs", ffi_rs.as_bytes()),
            (archive_name, archive.as_slice()),
            ("descriptor.json", descriptor_bytes),
        ],
    );
    verify_manifest(
        manifest.as_bytes(),
        &ManifestFacts {
            target,
            descriptor: descriptor_bytes,
            descriptor_digest,
            archive_name,
            files: [
                ("Cargo.toml", cargo_toml.as_bytes()),
                ("build.rs", build_rs.as_bytes()),
                ("lib.rs", lib_rs.as_bytes()),
                ("owned_data_ffi.rs", ffi_rs.as_bytes()),
                (archive_name, archive.as_slice()),
                ("descriptor.json", descriptor_bytes),
            ],
        },
    )?;
    publish_package(
        output,
        [
            ("Cargo.toml", cargo_toml.as_bytes()),
            ("build.rs", build_rs.as_bytes()),
            ("lib.rs", lib_rs.as_bytes()),
            ("owned_data_ffi.rs", ffi_rs.as_bytes()),
            (archive_name, archive.as_slice()),
            ("descriptor.json", descriptor_bytes),
            (
                "semaprax.native-rust-owned-data-sdk.json",
                manifest.as_bytes(),
            ),
        ],
    )?;
    verify_published_owned_data(
        output,
        [
            ("Cargo.toml", cargo_toml.as_bytes()),
            ("build.rs", build_rs.as_bytes()),
            ("lib.rs", lib_rs.as_bytes()),
            ("owned_data_ffi.rs", ffi_rs.as_bytes()),
            (archive_name, archive.as_slice()),
            ("descriptor.json", descriptor_bytes),
            (
                "semaprax.native-rust-owned-data-sdk.json",
                manifest.as_bytes(),
            ),
        ],
    )?;
    Ok(NativeRustOwnedDataSdkBundle {
        output_directory: output.to_path_buf(),
        manifest_path: output.join("semaprax.native-rust-owned-data-sdk.json"),
        manifest_digest: domain_digest(OWNED_MANIFEST_DOMAIN, manifest.as_bytes()),
        descriptor_digest: descriptor_digest.to_owned(),
        crate_name: OWNED_CRATE_NAME.to_owned(),
        target_triple: target.to_owned(),
    })
}

fn verify_published_owned_data(output: &Path, files: [(&str, &[u8]); 7]) -> Result<(), Diagnostic> {
    let directory = crate::platform::hold_directory(output).map_err(|_| publication_error())?;
    let held = files
        .iter()
        .map(|(name, bytes)| hold_matching(&directory, name, bytes, MAX_ARCHIVE_BYTES))
        .collect::<Result<Vec<_>, _>>()?;
    let names = files.map(|(name, _)| OsStr::new(name));
    let mut scan = crate::platform::prepare_inventory_entries_exact(names, 7)
        .map_err(|_| publication_error())?;
    crate::platform::inventory_entries_exact_prepared(
        &mut scan,
        &directory,
        [
            &held[0], &held[1], &held[2], &held[3], &held[4], &held[5], &held[6],
        ],
        [],
    )
    .map_err(|_| publication_error())?;
    crate::platform::recheck_directory(&directory).map_err(|_| publication_error())
}

fn render_sources(
    descriptor: &semaprax::project::PublicApiDescriptor,
    target: &str,
) -> (String, String, String, String) {
    let cargo = format!(
        "[package]\nname = \"{OWNED_CRATE_NAME}\"\nversion = \"{OWNED_CRATE_VERSION}\"\nedition = \"2021\"\nrust-version = \"1.85\"\npublish = false\nbuild = \"build.rs\"\n\n[lib]\npath = \"lib.rs\"\n\n[workspace]\n"
    );
    let archive = if cfg!(windows) {
        "semaprax_native_rust_owned_data_sdk.lib"
    } else {
        "libsemaprax_native_rust_owned_data_sdk.a"
    };
    let build_rs = format!(
        "#![forbid(unsafe_code)]\nfn main(){{if std::env::var(\"TARGET\").unwrap_or_default()!={target:?}{{panic!(\"generated SEMAPRAX owned-data SDK target mismatch\")}}println!(\"cargo:rerun-if-changed={archive}\");println!(\"cargo:rustc-link-search=native={{}}\",std::env::var(\"CARGO_MANIFEST_DIR\").unwrap());println!(\"cargo:rustc-link-lib=static=semaprax_native_rust_owned_data_sdk\");}}\n"
    );
    (
        cargo,
        build_rs,
        render_lib(descriptor),
        render_ffi(descriptor),
    )
}

fn render_lib(descriptor: &semaprax::project::PublicApiDescriptor) -> String {
    let mut output = String::from(
        "#[path=\"owned_data_ffi.rs\"]mod ffi;\nmod safe_api{#![forbid(unsafe_code)]\nuse super::ffi;\n#[derive(Clone,Copy,Debug,Eq,PartialEq)]pub enum CallError{SemanticFailure,AdapterRejected,HostFailure}\npub struct NativeRustOwnedDataSdk{context:ffi::Context}\nimpl NativeRustOwnedDataSdk{pub fn new()->Result<Self,CallError>{Ok(Self{context:ffi::Context::new().map_err(Self::map_failure)?})}\n",
    );
    output.push_str("fn map_failure(value:ffi::Failure)->CallError{match value{ffi::Failure::Semantic=>CallError::SemanticFailure,ffi::Failure::Adapter=>CallError::AdapterRejected,ffi::Failure::Host=>CallError::HostFailure}}\n");
    for export in descriptor.exports() {
        write!(output, "pub fn {}(&mut self", export.rust_method_name()).unwrap();
        for (index, parameter) in export.parameters().iter().enumerate() {
            write!(output, ",arg_{index}:{}", rust_parameter(parameter.ty())).unwrap();
        }
        write!(
            output,
            ")->Result<{},CallError>{{let raw=self.context.call_{}(",
            rust_result(export.result()),
            export.rust_method_name()
        )
        .unwrap();
        for index in 0..export.parameters().len() {
            if index != 0 {
                output.push(',');
            }
            write!(output, "arg_{index}").unwrap();
        }
        output.push_str(").map_err(Self::map_failure)?;");
        match export.result() {
            semaprax::project::PublicApiResultType::OwnedBytes => output.push_str("if raw.tag!=0||raw.handle==0{if raw.handle!=0{self.context.discard(raw.handle).map_err(map_failure)?}return Err(CallError::AdapterRejected)}self.context.copy_and_settle(raw.handle).map_err(map_failure)"),
            semaprax::project::PublicApiResultType::OptionOwnedBytes => output.push_str("match(raw.tag,raw.handle){(0,0)=>Ok(None),(1,handle)if handle!=0=>self.context.copy_and_settle(handle).map(Some).map_err(map_failure),(_,handle)=>{if handle!=0{self.context.discard(handle).map_err(map_failure)?}Err(CallError::AdapterRejected)}}"),
            semaprax::project::PublicApiResultType::ResultOwnedBytesI64 => output.push_str("match(raw.tag,raw.handle){(0,handle)if handle!=0=>self.context.copy_and_settle(handle).map(Ok).map_err(map_failure),(1,0)=>Ok(Err(raw.error)),(_,handle)=>{if handle!=0{self.context.discard(handle).map_err(map_failure)?}Err(CallError::AdapterRejected)}}"),
            _ => unreachable!("provider admission rejected scalar result"),
        }
        output.push_str("}\n");
    }
    output.push_str("}\n}\npub use safe_api::*;\n");
    output.replace("map_err(map_failure)", "map_err(Self::map_failure)")
}

fn render_ffi(descriptor: &semaprax::project::PublicApiDescriptor) -> String {
    let mut output = String::from(
        "#![allow(unsafe_code)]\nuse core::marker::PhantomData;use core::ptr::NonNull;use std::rc::Rc;\n#[repr(C)]struct RawContext{_private:[u8;0]}\ntype Handle=u64;type Status=u32;\nextern \"C\"{fn spx_owned_data_context_size_v1()->u64;fn spx_owned_data_context_align_v1()->u64;fn spx_owned_data_context_init_v1(storage:*mut core::ffi::c_void,length:u64)->Status;fn spx_owned_data_context_drop_v1(context:*mut RawContext)->Status;fn spx_owned_bytes_len_v1(context:*mut RawContext,handle:Handle,length:*mut u64)->Status;fn spx_owned_bytes_copy_v1(context:*mut RawContext,handle:Handle,destination:*mut u8,length:u64)->Status;fn spx_owned_bytes_drop_v1(context:*mut RawContext,handle:Handle)->Status;\n",
    );
    for export in descriptor.exports() {
        write!(
            output,
            "#[link_name={:?}]fn raw_{}(context:*mut RawContext",
            semaprax::codegen::native_owned_data_provider_symbol(export.rust_method_name()),
            export.rust_method_name()
        )
        .unwrap();
        for (index, parameter) in export.parameters().iter().enumerate() {
            match parameter.ty() {
                semaprax::project::PublicApiParameterType::I64 => {
                    write!(output, ",arg_{index}:i64")
                }
                semaprax::project::PublicApiParameterType::Bool => {
                    write!(output, ",arg_{index}:u8")
                }
                semaprax::project::PublicApiParameterType::BorrowStr
                | semaprax::project::PublicApiParameterType::BorrowSliceU8 => {
                    write!(output, ",arg_{index}:*const u8,arg_{index}_len:u64")
                }
            }
            .unwrap();
        }
        output.push_str(",tag:*mut u32,handle:*mut Handle,error:*mut i64)->Status;\n");
    }
    output.push_str("}\n#[derive(Clone,Copy)]pub(super)enum Failure{Semantic,Adapter,Host}\npub(super)struct RawCall{pub tag:u32,pub handle:Handle,pub error:i64}\npub(super)struct Context{storage:Vec<u64>,raw:NonNull<RawContext>,_thread:PhantomData<Rc<()>>}\nimpl Context{pub fn new()->Result<Self,Failure>{unsafe{let size=spx_owned_data_context_size_v1();let align=spx_owned_data_context_align_v1();if size==0||align==0||align>core::mem::align_of::<u64>()as u64{return Err(Failure::Adapter)}let rounded=size.checked_add(7).ok_or(Failure::Adapter)?;let words=usize::try_from(rounded/8).map_err(|_|Failure::Adapter)?;let mut storage=vec![0u64;words];let raw:NonNull<RawContext>=NonNull::new(storage.as_mut_ptr().cast()).ok_or(Failure::Host)?;if spx_owned_data_context_init_v1(raw.as_ptr().cast(),size)!=0{return Err(Failure::Adapter)}Ok(Self{storage,raw,_thread:PhantomData})}}\n");
    for export in descriptor.exports() {
        write!(
            output,
            "pub fn call_{}(&mut self",
            export.rust_method_name()
        )
        .unwrap();
        for (index, parameter) in export.parameters().iter().enumerate() {
            write!(output, ",arg_{index}:{}", rust_parameter(parameter.ty())).unwrap();
        }
        output.push_str(")->Result<RawCall,Failure>{let mut value=RawCall{tag:u32::MAX,handle:0,error:0};let status=unsafe{raw_");
        output.push_str(export.rust_method_name());
        output.push_str("(self.raw.as_ptr()");
        for (index, parameter) in export.parameters().iter().enumerate() {
            match parameter.ty() {
                semaprax::project::PublicApiParameterType::I64 => write!(output, ",arg_{index}"),
                semaprax::project::PublicApiParameterType::Bool => {
                    write!(output, ",u8::from(arg_{index})")
                }
                semaprax::project::PublicApiParameterType::BorrowStr => {
                    write!(output, ",arg_{index}.as_ptr(),arg_{index}.len()as u64")
                }
                semaprax::project::PublicApiParameterType::BorrowSliceU8 => {
                    write!(output, ",arg_{index}.as_ptr(),arg_{index}.len()as u64")
                }
            }
            .unwrap();
        }
        output.push_str(",&mut value.tag,&mut value.handle,&mut value.error)};if status!=0&&value.handle!=0{self.discard(value.handle)?}match status{0=>Ok(value),1=>Err(Failure::Semantic),2..=5=>Err(Failure::Adapter),_=>Err(Failure::Host)}}\n");
    }
    output.push_str("pub fn copy_and_settle(&mut self,handle:Handle)->Result<Vec<u8>,Failure>{let mut guard=Guard{context:self,handle,armed:true};let mut length=0u64;if unsafe{spx_owned_bytes_len_v1(guard.context.raw.as_ptr(),handle,&mut length)}!=0{return Err(Failure::Adapter)}if length>65536{return Err(Failure::Adapter)}let length=usize::try_from(length).map_err(|_|Failure::Adapter)?;let mut bytes=vec![0u8;length];if bytes.capacity()!=length{return Err(Failure::Host)}let pointer=if length==0{core::ptr::null_mut()}else{bytes.as_mut_ptr()};if unsafe{spx_owned_bytes_copy_v1(guard.context.raw.as_ptr(),handle,pointer,length as u64)}!=0{return Err(Failure::Adapter)}if unsafe{spx_owned_bytes_drop_v1(guard.context.raw.as_ptr(),handle)}!=0{std::process::abort()}guard.armed=false;Ok(bytes)}pub fn discard(&mut self,handle:Handle)->Result<(),Failure>{if unsafe{spx_owned_bytes_drop_v1(self.raw.as_ptr(),handle)}!=0{std::process::abort()}Ok(())}}\nstruct Guard<'a>{context:&'a mut Context,handle:Handle,armed:bool}impl Drop for Guard<'_>{fn drop(&mut self){if self.armed&&unsafe{spx_owned_bytes_drop_v1(self.context.raw.as_ptr(),self.handle)}!=0{std::process::abort()}}}\nimpl Drop for Context{fn drop(&mut self){let _=self.storage.len();if unsafe{spx_owned_data_context_drop_v1(self.raw.as_ptr())}!=0{std::process::abort()}}}\n");
    output
}

fn rust_parameter(ty: semaprax::project::PublicApiParameterType) -> &'static str {
    match ty {
        semaprax::project::PublicApiParameterType::I64 => "i64",
        semaprax::project::PublicApiParameterType::Bool => "bool",
        semaprax::project::PublicApiParameterType::BorrowStr => "&str",
        semaprax::project::PublicApiParameterType::BorrowSliceU8 => "&[u8]",
    }
}

fn rust_result(ty: semaprax::project::PublicApiResultType) -> &'static str {
    match ty {
        semaprax::project::PublicApiResultType::OwnedBytes => "Vec<u8>",
        semaprax::project::PublicApiResultType::OptionOwnedBytes => "Option<Vec<u8>>",
        semaprax::project::PublicApiResultType::ResultOwnedBytesI64 => "Result<Vec<u8>,i64>",
        _ => unreachable!("provider admission rejected scalar result"),
    }
}

fn build_archive(provider: &[u8], target: &str, root: &Path) -> Result<Vec<u8>, Diagnostic> {
    let parent = crate::platform::hold_directory(root).map_err(|_| publication_error())?;
    let name = format!(
        ".semaprax-owned-data-provider-{}-{}",
        std::process::id(),
        STAGE_NONCE.fetch_add(1, Ordering::Relaxed)
    );
    let prepared =
        crate::platform::prepare_stage_name(OsStr::new(&name)).map_err(|_| publication_error())?;
    let path = root.join(&name);
    let object_name = if cfg!(windows) {
        "module.obj"
    } else {
        "module.o"
    };
    let archive_name = if cfg!(windows) {
        "semaprax_native_rust_sdk.lib"
    } else {
        "libsemaprax_native_rust_sdk.a"
    };
    let inventory = crate::platform::prepare_discard_inventory([
        OsStr::new("provider.c"),
        OsStr::new(object_name),
        OsStr::new(archive_name),
    ])
    .map_err(|_| publication_error())?;
    let directory = crate::platform::create_directory_new_prepared(&parent, &prepared, 0o700)
        .map_err(|_| publication_error())?;
    let mut inventory = inventory;
    let result = (|| {
        if !crate::platform::same_directory_path(&directory, &path)
            .map_err(|_| publication_error())?
        {
            return Err(publication_error());
        }
        crate::platform::write_file_new_prepared(
            &directory,
            &mut inventory,
            "provider.c",
            provider,
            0o600,
        )
        .map_err(|_| publication_error())?;
        crate::platform::transition_regular_file_to_external_read_prepared(
            &directory,
            &mut inventory,
            "provider.c",
        )
        .map_err(|_| publication_error())?;
        let clang = crate::platform::hold_configured_tool("CLANG", "clang")
            .map_err(|_| publication_error())?;
        let mut compile_arena = crate::platform::materialize_process_arena(
            crate::platform::prepare_process_arena_plan(1).map_err(|_| publication_error())?,
        )
        .map_err(|_| publication_error())?;
        let compile = crate::platform::prepare_c_compile_invocation(
            target,
            OsStr::new("provider.c"),
            2,
            false,
            MAX_PROVIDER_BYTES,
        )
        .map_err(|_| publication_error())?;
        let object = crate::platform::compile_c_tool_prepared(
            &clang,
            &directory,
            compile,
            &mut compile_arena,
        )
        .map_err(|_| publication_error())?
        .into_bytes();
        crate::platform::write_file_new_prepared(
            &directory,
            &mut inventory,
            object_name,
            &object,
            0o600,
        )
        .map_err(|_| publication_error())?;
        #[cfg(windows)]
        crate::platform::transition_regular_file_to_external_read_prepared(
            &directory,
            &mut inventory,
            object_name,
        )
        .map_err(|_| publication_error())?;
        let archiver_path = std::env::var_os("SEMAPRAX_ARCHIVER")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .ok_or_else(publication_error)?;
        #[cfg(windows)]
        let vctools_path = std::env::var_os("SEMAPRAX_VCTOOLS")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .ok_or_else(publication_error)?;
        #[cfg(windows)]
        let vctools = Some(vctools_path.as_path());
        #[cfg(not(windows))]
        let vctools: Option<&Path> = None;
        let archiver = crate::platform::hold_configured_archiver(archiver_path, vctools)
            .map_err(|_| publication_error())?;
        let mut arena = crate::platform::materialize_process_arena(
            crate::platform::prepare_process_arena_plan(1).map_err(|_| publication_error())?,
        )
        .map_err(|_| publication_error())?;
        let invocation = crate::platform::prepare_archive_invocation(
            OsStr::new(object_name),
            OsStr::new(archive_name),
        )
        .map_err(|_| publication_error())?;
        let archive = crate::platform::archive_tool_prepared(
            &archiver,
            &directory,
            inventory
                .file(object_name)
                .map_err(|_| publication_error())?,
            invocation,
            &mut arena,
        )
        .map_err(|_| publication_error())?;
        inventory
            .attach(archive_name, archive)
            .map_err(|_| publication_error())?;
        let bytes = crate::platform::read_exact(
            inventory
                .file(archive_name)
                .map_err(|_| publication_error())?,
            MAX_ARCHIVE_BYTES,
        )
        .map_err(|_| publication_error())?;
        Ok(bytes)
    })();
    let cleanup =
        crate::platform::discard_owned_stage_prepared(&parent, &directory, &prepared, &inventory);
    match (result, cleanup) {
        (Ok(bytes), Ok(())) => Ok(bytes),
        (Err(error), _) => Err(error),
        (Ok(_), Err(_)) => Err(publication_error()),
    }
}

fn publish_package(output: &Path, files: [(&'static str, &[u8]); 7]) -> Result<(), Diagnostic> {
    if !output.is_absolute() {
        return Err(publication_error());
    }
    let parent_path = output.parent().ok_or_else(publication_error)?;
    let output_name = output.file_name().ok_or_else(publication_error)?;
    let parent = crate::platform::hold_directory(parent_path).map_err(|_| publication_error())?;
    let probe =
        crate::platform::prepare_child_name(output_name).map_err(|_| publication_error())?;
    if !crate::platform::child_absent_prepared(&parent, &probe).map_err(|_| publication_error())? {
        return Err(publication_error());
    }
    let stage_name_text = format!(
        ".semaprax-owned-data-sdk-{}-{}",
        std::process::id(),
        STAGE_NONCE.fetch_add(1, Ordering::Relaxed)
    );
    let stage_name = crate::platform::prepare_stage_name(OsStr::new(&stage_name_text))
        .map_err(|_| publication_error())?;
    let stage_path = parent_path.join(&stage_name_text);
    let names = files.map(|(name, _)| OsStr::new(name));
    let inventory =
        crate::platform::prepare_discard_inventory(names).map_err(|_| publication_error())?;
    let stage = crate::platform::create_directory_new_prepared(&parent, &stage_name, 0o700)
        .map_err(|_| publication_error())?;
    let mut inventory = inventory;
    let result = (|| {
        if !crate::platform::same_directory_path(&stage, &stage_path)
            .map_err(|_| publication_error())?
        {
            return Err(publication_error());
        }
        for (name, bytes) in files {
            crate::platform::write_file_new_prepared(&stage, &mut inventory, name, bytes, 0o600)
                .map_err(|_| publication_error())?;
        }
        let mut scan = crate::platform::prepare_inventory_exact(&inventory)
            .map_err(|_| publication_error())?;
        authenticate_inventory(&mut scan, &stage, &inventory)?;
        inventory
            .settle_for_publish()
            .map_err(|_| publication_error())?;
        let mut publish = crate::platform::prepare_publish_directory(output_name)
            .map_err(|_| publication_error())?;
        crate::platform::publish_directory_new_prepared(
            &mut publish,
            &parent,
            &stage,
            &stage_name,
            output_name,
        )
        .map_err(|_| publication_error())
    })();
    if result.is_err() {
        let _ =
            crate::platform::discard_owned_stage_prepared(&parent, &stage, &stage_name, &inventory);
    }
    result
}

pub(super) fn render_manifest(
    target: &str,
    descriptor: &[u8],
    descriptor_digest: &str,
    archive_name: &str,
    mut files: [(&str, &[u8]); 6],
) -> String {
    files.sort_by_key(|row| row.0.as_bytes());
    let mut output = String::new();
    output.push_str("{\"schema\":");
    json_string(&mut output, NATIVE_RUST_OWNED_DATA_SDK_SCHEMA);
    output.push_str(",\"crate\":{\"name\":");
    json_string(&mut output, OWNED_CRATE_NAME);
    output.push_str(",\"version\":");
    json_string(&mut output, OWNED_CRATE_VERSION);
    output.push('}');
    output.push_str(",\"target\":");
    json_string(&mut output, target);
    output.push_str(",\"descriptor\":{\"schema\":");
    json_string(&mut output, semaprax::project::PUBLIC_OWNED_DATA_API_SCHEMA);
    write!(output, ",\"bytes\":{},\"digest\":", descriptor.len()).unwrap();
    json_string(&mut output, descriptor_digest);
    output.push('}');
    output.push_str(",\"provider\":{\"abi\":\"opaque-handle.v1\",\"archive\":");
    json_string(&mut output, archive_name);
    output.push_str(",\"operations\":[\"len\",\"copy\",\"drop\"]}");
    output.push_str(",\"files\":[");
    for (index, (path, bytes)) in files.iter().enumerate() {
        if index != 0 {
            output.push(',')
        }
        output.push_str("{\"path\":");
        json_string(&mut output, path);
        write!(output, ",\"bytes\":{},\"sha256\":", bytes.len()).unwrap();
        json_string(&mut output, &raw_digest(bytes));
        output.push('}')
    }
    output.push(']');
    output.push_str(",\"limits\":{\"max_borrowed_input_bytes\":65536,\"max_owned_output_bytes\":65536,\"max_handles\":4096,\"exact_package_files\":7}");
    output.push_str(",\"nonclaims\":[\"no_raw_handle_or_context_public_api\",\"no_allocator_transfer\",\"no_allocator_oom_abort_or_panic_recovery_proof\",\"no_send_sync\",\"no_project_v8_activation\"]}\n");
    output
}

fn publication_error() -> Diagnostic {
    Diagnostic::io("SPX-I234", "Native Rust owned-data SDK publication failed")
}
fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-B114", message)
}
