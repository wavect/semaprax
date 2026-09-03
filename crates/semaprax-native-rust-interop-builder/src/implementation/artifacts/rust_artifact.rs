//! Generated safe-Rust and private-FFI projections.

use super::*;

fn rust_parameters(parameters: &[ParameterFact]) -> String {
    let values = parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| format!("arg_{index}: {}", rust_type(parameter.ty)))
        .collect::<Vec<_>>();
    let joined = values.join(", ");
    #[cfg(test)]
    note_post_hir_render_capacity(
        string_slice_owned_capacity(&values).saturating_add(joined.capacity()),
    );
    joined
}

fn generate_safe_rust_into(
    output: &mut dyn std::fmt::Write,
    spec: &Spec,
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<(), Diagnostic> {
    output.write_str("mod api{#![forbid(unsafe_code)]\nuse core::num::NonZeroU32;\n#[repr(u8)] #[derive(Clone,Copy,Debug,Eq,PartialEq)] pub enum NativeRustStatusClass{Semantic=1,Contract=2,Import=3,Adapter=4}\n").unwrap();
    if !imports.is_empty() {
        output.write_str("pub enum NativeRustImportResult<T>{Success(T),Status{code:NonZeroU32,class:NativeRustStatusClass,retryable:bool},HostFailure}\n").unwrap();
    }
    output.write_str("pub enum NativeRustCallError{Semantic{domain_id:&'static str,code:NonZeroU32,class:NativeRustStatusClass,retryable:bool},HostFailed,HostPanicked,AdapterRejected}\npub struct NativeRustAdmissionError;\n").unwrap();
    output.write_str("pub trait NativeRustImports{").unwrap();
    for import in imports {
        write!(
            output,
            "fn {}(&mut self{}{})->NativeRustImportResult<{}>;",
            import.rust_method,
            if import.parameters.is_empty() {
                ""
            } else {
                ", "
            },
            rust_parameters(&import.parameters),
            rust_type(import.result)
        )
        .unwrap();
    }
    output.write_str("}\n").unwrap();
    let capability_values = spec
        .capabilities
        .iter()
        .map(|value| quote_json(value))
        .collect::<Vec<_>>();
    let capabilities = capability_values.join(",");
    #[cfg(test)]
    note_post_hir_render_capacity(
        string_slice_owned_capacity(&capability_values).saturating_add(capabilities.capacity()),
    );
    write!(
        output,
        "const EXPECTED_CAPABILITIES:&[&str]=&[{}];\n",
        capabilities
    )
    .unwrap();
    output.write_str("pub struct NativeRustCapabilities{digest:[u8;32]} impl NativeRustCapabilities{pub fn new(values:&[&str])->Result<Self,NativeRustAdmissionError>{if values!=EXPECTED_CAPABILITIES{return Err(NativeRustAdmissionError)}Ok(Self{digest:super::ffi::capabilities_digest()})}}\n").unwrap();
    output.write_str("struct ActiveGuard<'a>{active:&'a mut bool}impl Drop for ActiveGuard<'_>{fn drop(&mut self){*self.active=false;}}\npub struct NativeRustBridge<H:NativeRustImports>{host:H,capabilities:NativeRustCapabilities,owner:std::thread::ThreadId,active:bool,calls:u32,_not_send_sync:core::marker::PhantomData<*mut ()>} impl<H:NativeRustImports> NativeRustBridge<H>{pub fn new(host:H,capabilities:NativeRustCapabilities)->Self{Self{host,capabilities,owner:std::thread::current().id(),active:false,calls:0,_not_send_sync:core::marker::PhantomData}}\n").unwrap();
    for export in exports {
        let parameters = rust_parameters(&export.parameters);
        let argument_values = (0..export.parameters.len())
            .map(|index| format!("arg_{index}"))
            .collect::<Vec<_>>();
        let arguments = argument_values.join(", ");
        #[cfg(test)]
        note_post_hir_render_capacity(
            parameters
                .capacity()
                .saturating_add(string_slice_owned_capacity(&argument_values))
                .saturating_add(arguments.capacity()),
        );
        write!(output,"pub fn {}(&mut self{}{})->Result<{},NativeRustCallError>{{if self.owner!=std::thread::current().id()||core::mem::replace(&mut self.active,true){{return Err(NativeRustCallError::AdapterRejected)}}let _active_guard=ActiveGuard{{active:&mut self.active}};super::ffi::{}(&mut self.host,&mut self.calls,self.capabilities.digest{}{})}}\n",export.rust_method,if export.parameters.is_empty(){""}else{", "},parameters,rust_type(export.result),export.rust_method,if export.parameters.is_empty(){""}else{", "},arguments).unwrap();
    }
    output
        .write_str(
            "}\n}\n#[path=\"semaprax_native_rust_interop_ffi.rs\"]mod ffi;\npub use api::*;\n",
        )
        .unwrap();
    Ok(())
}

fn generate_private_ffi_into(
    output: &mut dyn std::fmt::Write,
    spec: &Spec,
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<(), Diagnostic> {
    let digest = capability_digest(&spec.capabilities);
    let hex = digest.strip_prefix("sha256:").unwrap_or("");
    let byte_values = (0..64)
        .step_by(2)
        .map(|index| format!("0x{}", &hex[index..index + 2]))
        .collect::<Vec<_>>();
    let bytes = byte_values.join(",");
    let mut import_table_values = Vec::with_capacity(imports.len());
    for import in imports {
        let parameter_values = import
            .parameters
            .iter()
            .map(|parameter| match parameter.ty {
                ScalarType::I64 => "i64".to_owned(),
                ScalarType::Bool => "u8".to_owned(),
                ScalarType::Unit => "()".to_owned(),
            })
            .collect::<Vec<_>>();
        let parameters = parameter_values.join(",");
        let result = if import.result == ScalarType::Unit {
            String::new()
        } else {
            format!(", *mut {}", rust_ffi_wire_type(import.result))
        };
        let row = format!(
            "{}:unsafe extern \"C\" fn(*mut c_void{}{}{})->u64,",
            import.c_field,
            if import.parameters.is_empty() {
                ""
            } else {
                ", "
            },
            parameters,
            result
        );
        #[cfg(test)]
        note_post_hir_render_capacity(
            string_slice_owned_capacity(&byte_values)
                .saturating_add(bytes.capacity())
                .saturating_add(string_slice_owned_capacity(&import_table_values))
                .saturating_add(string_slice_owned_capacity(&parameter_values))
                .saturating_add(parameters.capacity())
                .saturating_add(result.capacity())
                .saturating_add(row.capacity()),
        );
        import_table_values.push(row);
    }
    let import_table = import_table_values.join("");
    #[cfg(test)]
    note_post_hir_render_capacity(
        string_slice_owned_capacity(&byte_values)
            .saturating_add(bytes.capacity())
            .saturating_add(string_slice_owned_capacity(&import_table_values))
            .saturating_add(import_table.capacity()),
    );
    write!(output, "#![allow(unsafe_code)]\nuse super::api::*;\nuse core::ffi::c_void;\n#[repr(C)]struct Imports{{abi_version:u32,size:u32,{import_table} }}\n#[repr(C)]struct Context{{abi_version:u32,size:u32,userdata:*mut c_void,imports:*const Imports,capabilities_digest:[u8;32],call_depth:u32,reserved:u32}}\n").unwrap();
    if !imports.is_empty() {
        output
            .write_str("struct Frame<H>{host:*mut H,calls:*mut u32}\n")
            .unwrap();
    }
    write!(
        output,
        "pub(super) fn capabilities_digest()->[u8;32]{{[{bytes}]}}\n"
    )
    .unwrap();
    #[cfg(test)]
    let ffi_prefix_scratch = digest
        .capacity()
        .saturating_add(string_slice_owned_capacity(&byte_values))
        .saturating_add(bytes.capacity())
        .saturating_add(string_slice_owned_capacity(&import_table_values))
        .saturating_add(import_table.capacity());
    if !imports.is_empty() {
        output.write_str("fn adapter(code:u32)->u64{((65535u64)<<48)|((NativeRustStatusClass::Adapter as u64)<<32)|u64::from(code)}\n").unwrap();
    }
    output.write_str("fn decode_status(status:u64)->NativeRustCallError{let code=(status&0xffff_ffff)as u32;let class=((status>>32)&0xff)as u8;let retryable=((status>>40)&1)!=0;let reserved=(status>>41)&0x7f;let domain=(status>>48)as u16;let Some(code)=core::num::NonZeroU32::new(code)else{return NativeRustCallError::AdapterRejected};let class=match class{1=>NativeRustStatusClass::Semantic,2=>NativeRustStatusClass::Contract,3=>NativeRustStatusClass::Import,4=>NativeRustStatusClass::Adapter,_=>return NativeRustCallError::AdapterRejected};if reserved!=0||domain==0{return NativeRustCallError::AdapterRejected}match domain{65533=>{let valid=!retryable&&match class{NativeRustStatusClass::Semantic=>(1..=6).contains(&code.get()),NativeRustStatusClass::Contract=>(1..=2).contains(&code.get()),_=>false};if !valid{return NativeRustCallError::AdapterRejected}NativeRustCallError::Semantic{domain_id:\"semaprax.native-rust-semantics.v1\",code,class,retryable}},").unwrap();
    let domains = imports
        .iter()
        .filter_map(|import| import.failure.as_ref())
        .cloned()
        .collect::<BTreeSet<_>>();
    for (index, domain) in domains.iter().enumerate() {
        write!(output,"{}=>{{if class!=NativeRustStatusClass::Import{{return NativeRustCallError::AdapterRejected}}NativeRustCallError::Semantic{{domain_id:{},code,class,retryable}}}},",index+1,quote_json(domain)).unwrap();
    }
    output.write_str("65534=>if class==NativeRustStatusClass::Adapter&&!retryable{match code.get(){1=>NativeRustCallError::HostPanicked,2=>NativeRustCallError::HostFailed,_=>NativeRustCallError::AdapterRejected}}else{NativeRustCallError::AdapterRejected},65535=>if class==NativeRustStatusClass::Adapter&&!retryable&&(1..=8).contains(&code.get()){NativeRustCallError::AdapterRejected}else{NativeRustCallError::AdapterRejected},_=>NativeRustCallError::AdapterRejected}}\n").unwrap();
    for import in imports {
        let parameter_declaration_values = import
            .parameters
            .iter()
            .enumerate()
            .map(|(index, p)| {
                format!(
                    "arg_{index}:{}",
                    match p.ty {
                        ScalarType::I64 => "i64",
                        ScalarType::Bool => "u8",
                        ScalarType::Unit => "()",
                    }
                )
            })
            .collect::<Vec<_>>();
        let parameter_declarations = parameter_declaration_values.join(",");
        let result_declaration = if import.result == ScalarType::Unit {
            String::new()
        } else {
            format!(
                ", result_out:*mut {}",
                match import.result {
                    ScalarType::I64 => "i64",
                    ScalarType::Bool => "u8",
                    ScalarType::Unit => "()",
                }
            )
        };
        #[cfg(test)]
        note_post_hir_render_capacity(
            ffi_prefix_scratch
                .saturating_add(owned_string_set_owned_capacity(&domains))
                .saturating_add(string_slice_owned_capacity(&parameter_declaration_values))
                .saturating_add(parameter_declarations.capacity())
                .saturating_add(result_declaration.capacity()),
        );
        write!(output,"unsafe extern \"C\" fn cb_{}<H:NativeRustImports>(userdata:*mut c_void{}{}{}) -> u64{{if userdata.is_null(){{return adapter(1);}}",import.rust_method,if import.parameters.is_empty(){""}else{", "},parameter_declarations,result_declaration).unwrap();
        for (index, parameter) in import.parameters.iter().enumerate() {
            if parameter.ty == ScalarType::Bool {
                write!(output, "if arg_{index}>1{{return adapter(4);}}").unwrap();
            }
        }
        if import.result != ScalarType::Unit {
            write!(output,"if result_out.is_null()||(result_out as usize)%core::mem::align_of::<{}>()!=0{{return adapter(5);}}",rust_type(import.result)).unwrap();
        }
        let call_argument_values = import
            .parameters
            .iter()
            .enumerate()
            .map(|(index, p)| {
                if p.ty == ScalarType::Bool {
                    format!("arg_{index}!=0")
                } else {
                    format!("arg_{index}")
                }
            })
            .collect::<Vec<_>>();
        let call_arguments = call_argument_values.join(",");
        #[cfg(test)]
        note_post_hir_render_capacity(
            ffi_prefix_scratch
                .saturating_add(owned_string_set_owned_capacity(&domains))
                .saturating_add(string_slice_owned_capacity(&call_argument_values))
                .saturating_add(call_arguments.capacity()),
        );
        write!(output,"if (userdata as usize)%core::mem::align_of::<Frame<H>>()!=0{{return adapter(1);}}let frame=&mut*(userdata as *mut Frame<H>);if frame.host.is_null()||frame.calls.is_null()||*frame.calls>=4096{{return adapter(7);}}*frame.calls+=1;let run=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{{let host=&mut *frame.host;host.{}({})}}));match run{{Err(payload)=>{{core::mem::forget(payload);((65534u64)<<48)|((NativeRustStatusClass::Adapter as u64)<<32)|1}},Ok(NativeRustImportResult::HostFailure)=>((65534u64)<<48)|((NativeRustStatusClass::Adapter as u64)<<32)|2,",import.rust_method,call_arguments).unwrap();
        let ordinal = import
            .failure
            .as_ref()
            .and_then(|domain| domains.iter().position(|value| value == domain))
            .map(|index| index + 1);
        if let Some(ordinal) = ordinal {
            write!(output,"Ok(NativeRustImportResult::Status{{code,class,retryable}})=>if class==NativeRustStatusClass::Import{{(({}u64)<<48)|((class as u64)<<32)|((retryable as u64)<<40)|u64::from(code.get())}}else{{adapter(3)}},",ordinal).unwrap();
        } else {
            output.write_str("Ok(NativeRustImportResult::Status{code,class,retryable})=>{let _=(code,class,retryable);adapter(3)},").unwrap();
        }
        if import.result == ScalarType::Unit {
            output
                .write_str("Ok(NativeRustImportResult::Success(()))=>0}}}\n")
                .unwrap();
        } else {
            write!(
                output,
                "Ok(NativeRustImportResult::Success(value))=>{{*result_out={};0}}",
                if import.result == ScalarType::Bool {
                    "u8::from(value)"
                } else {
                    "value"
                }
            )
            .unwrap();
            output.write_str("}}\n").unwrap();
        }
    }
    for export in exports {
        let parameter_values = export
            .parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| format!("arg_{index}:{}", rust_ffi_wire_type(parameter.ty)))
            .collect::<Vec<_>>();
        let parameters = parameter_values.join(",");
        let result = if export.result == ScalarType::Unit {
            String::new()
        } else {
            format!(", result_out:*mut {}", rust_ffi_wire_type(export.result))
        };
        #[cfg(test)]
        note_post_hir_render_capacity(
            ffi_prefix_scratch
                .saturating_add(owned_string_set_owned_capacity(&domains))
                .saturating_add(string_slice_owned_capacity(&parameter_values))
                .saturating_add(parameters.capacity())
                .saturating_add(result.capacity()),
        );
        write!(
            output,
            "extern \"C\"{{fn {}(ctx:*const Context{}{}{})->u64;}}\n",
            export.c_symbol,
            if export.parameters.is_empty() {
                ""
            } else {
                ", "
            },
            parameters,
            result
        )
        .unwrap();
    }
    for export in exports {
        let result_slot = match export.result {
            ScalarType::Unit => String::new(),
            ScalarType::I64 => "let mut result=core::mem::MaybeUninit::<i64>::uninit();".to_owned(),
            ScalarType::Bool => "let mut result=core::mem::MaybeUninit::<u8>::uninit();".to_owned(),
        };
        let publish = match export.result {
            ScalarType::Unit => "Ok(())",
            ScalarType::I64 => "Ok(result.assume_init())",
            ScalarType::Bool => {
                "let value=result.assume_init();if value>1{return Err(NativeRustCallError::AdapterRejected)}Ok(value!=0)"
            }
        };
        let parameters = rust_parameters(&export.parameters);
        let callback_values = imports
            .iter()
            .map(|import| format!("{}:cb_{}::<H>,", import.c_field, import.rust_method))
            .collect::<Vec<_>>();
        let callbacks = callback_values.join("");
        let argument_values = export
            .parameters
            .iter()
            .enumerate()
            .map(|(index, p)| {
                if p.ty == ScalarType::Bool {
                    format!("u8::from(arg_{index})")
                } else {
                    format!("arg_{index}")
                }
            })
            .collect::<Vec<_>>();
        let arguments = argument_values.join(",");
        let result_argument = if export.result == ScalarType::Unit {
            String::new()
        } else {
            ", result.as_mut_ptr()".to_owned()
        };
        #[cfg(test)]
        note_post_hir_render_capacity(
            ffi_prefix_scratch
                .saturating_add(owned_string_set_owned_capacity(&domains))
                .saturating_add(
                    parameters
                        .capacity()
                        .saturating_add(string_slice_owned_capacity(&callback_values))
                        .saturating_add(callbacks.capacity())
                        .saturating_add(result_slot.capacity())
                        .saturating_add(string_slice_owned_capacity(&argument_values))
                        .saturating_add(arguments.capacity())
                        .saturating_add(result_argument.capacity()),
                ),
        );
        let frame = if imports.is_empty() {
            "let _=host;let ctx=Context{abi_version:1,size:core::mem::size_of::<Context>() as u32,userdata:core::ptr::null_mut(),imports:&table,capabilities_digest:digest,call_depth:0,reserved:0};"
        } else {
            "let mut frame=Frame{host:host as *mut H,calls:calls as *mut u32};let ctx=Context{abi_version:1,size:core::mem::size_of::<Context>() as u32,userdata:&mut frame as *mut Frame<H> as *mut c_void,imports:&table,capabilities_digest:digest,call_depth:0,reserved:0};"
        };
        write!(output,"pub(super) fn {}<H:NativeRustImports>(host:&mut H,calls:&mut u32,digest:[u8;32]{}{})->Result<{},NativeRustCallError>{{unsafe{{if *calls>=4096{{return Err(NativeRustCallError::AdapterRejected)}}*calls+=1;let table=Imports{{abi_version:1,size:core::mem::size_of::<Imports>() as u32,{}}};{}{}let status={}(&ctx{}{}{});if status!=0{{return Err(decode_status(status))}}{} }}}}\n",export.rust_method,if export.parameters.is_empty(){""}else{", "},parameters,rust_type(export.result),callbacks,frame,result_slot,export.c_symbol,if export.parameters.is_empty(){""}else{", "},arguments,result_argument,publish).unwrap();
    }
    Ok(())
}

pub(in crate::implementation) fn generate_rust_artifacts_with_limit(
    spec: &Spec,
    exports: &[ExportFact],
    imports: &[ImportFact],
    maximum: usize,
) -> Result<(String, String), Diagnostic> {
    let mut render_safe =
        |sink: &mut dyn std::fmt::Write| generate_safe_rust_into(sink, spec, exports, imports);
    let mut render_ffi =
        |sink: &mut dyn std::fmt::Write| generate_private_ffi_into(sink, spec, exports, imports);
    let safe_bytes = count_exact_artifact("max_generated_rust_bytes", maximum, &mut render_safe)?;
    let ffi_bytes = count_exact_artifact("max_generated_rust_bytes", maximum, &mut render_ffi)?;
    let combined_bytes = safe_bytes
        .checked_add(ffi_bytes)
        .ok_or_else(|| b109("max_generated_rust_bytes", maximum))?;
    if combined_bytes > maximum {
        return Err(b109("max_generated_rust_bytes", maximum));
    }
    let safe = render_counted_artifact(
        "max_generated_rust_bytes",
        maximum,
        safe_bytes,
        &mut render_safe,
    )?;
    let ffi = render_counted_artifact(
        "max_generated_rust_bytes",
        maximum,
        ffi_bytes,
        &mut render_ffi,
    )?;
    Ok((safe, ffi))
}
