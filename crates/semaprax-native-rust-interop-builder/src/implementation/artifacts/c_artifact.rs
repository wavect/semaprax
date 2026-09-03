//! Generated C translation-unit projection and the capability digest.

use super::*;

pub(in crate::implementation) fn generate_c_into(
    output: &mut dyn std::fmt::Write,
    spec: &Spec,
    closure: &[&ResolvedFunction],
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<(), Diagnostic> {
    let capability_digest = capability_digest(&spec.capabilities);
    let capability_hex = capability_digest.strip_prefix("sha256:").ok_or_else(b111)?;
    let bytes = (0..64)
        .step_by(2)
        .map(|index| format!("0x{}", &capability_hex[index..index + 2]))
        .collect::<Vec<_>>()
        .join(",");
    write!(
        output,
        "#include \"semaprax_native_rust_interop.h\"\n#include <stdint.h>\n#include <stddef.h>\n#include <string.h>\n#include <limits.h>\nstatic const uint8_t spxnr_capabilities[32] = {{{bytes}}};\nstatic spxnr_status_v1 spxnr_adapter(uint32_t code){{return (((uint64_t)65535)<<48)|(((uint64_t)4)<<32)|code;}}\nstatic spxnr_status_v1 spxnr_validate(const spxnr_context_v1 *ctx){{if(!ctx||((uintptr_t)ctx%_Alignof(spxnr_context_v1))!=0)return spxnr_adapter(1);if(ctx->abi_version!=1||ctx->size!=sizeof(*ctx)||ctx->reserved!=0)return spxnr_adapter(1);if(!ctx->imports||((uintptr_t)ctx->imports%_Alignof(spxnr_imports_v1))!=0)return spxnr_adapter(2);if(ctx->imports->abi_version!=1||ctx->imports->size!=sizeof(*ctx->imports))return spxnr_adapter(2);if(memcmp(ctx->capabilities_digest,spxnr_capabilities,32)!=0)return spxnr_adapter(3);if(ctx->call_depth>=32)return spxnr_adapter(7);return 0;}}\n"
    )
    .unwrap();
    if !imports.is_empty() {
        output.write_str("static int spxnr_status_canonical(spxnr_status_v1 status){if(status==0)return 1;uint32_t code=(uint32_t)status;uint8_t class_=(uint8_t)(status>>32);uint8_t retry=(uint8_t)((status>>40)&1);uint8_t reserved=(uint8_t)((status>>41)&0x7f);uint16_t domain=(uint16_t)(status>>48);if(code==0||reserved!=0||domain==0)return 0;if(domain==65533)return retry==0&&((class_==1&&code>=1&&code<=6)||(class_==2&&code>=1&&code<=2));").unwrap();
    }
    let domains = imports
        .iter()
        .filter_map(|import| import.failure.as_ref())
        .collect::<BTreeSet<_>>();
    for (index, _) in domains.iter().enumerate() {
        write!(output, "if(domain=={})return class_==3;", index + 1).unwrap();
    }
    if !imports.is_empty() {
        output.write_str("if(domain==65534)return class_==4&&retry==0&&code>=1&&code<=2;if(domain==65535)return class_==4&&retry==0&&code>=1&&code<=8;return 0;}\n").unwrap();
    }
    let domain_ordinals = domains
        .iter()
        .enumerate()
        .map(|(index, domain)| (domain.as_str(), index + 1))
        .collect::<BTreeMap<_, _>>();
    for import in imports {
        let custom = import
            .failure
            .as_deref()
            .and_then(|domain| domain_ordinals.get(domain).copied());
        write!(
            output,
            "static int spxnr_status_for_{}(spxnr_status_v1 status){{if(!spxnr_status_canonical(status))return 0;uint16_t domain=(uint16_t)(status>>48);return domain==65534||domain==65535{};}}\n",
            import.rust_method,
            custom.map_or_else(String::new, |ordinal| format!("||domain=={ordinal}"))
        )
        .unwrap();
        write!(output,"static spxnr_status_v1 spxnr_validate_{}(const spxnr_context_v1 *ctx){{return ctx->imports->{}?0:spxnr_adapter(2);}}\n",import.rust_method,import.c_field).unwrap();
    }
    for function in closure {
        let parameters = parameter_facts(function)?;
        let result = scalar_type(&function.return_type).ok_or_else(b111)?;
        let params = c_parameters(&parameters);
        write!(
            output,
            "static spxnr_status_v1 spxnr1_f_{}(const spxnr_context_v1 *ctx{}{}{});\n",
            full_hash(function.id.as_str()),
            if params.is_empty() { "" } else { ", " },
            params,
            if result == ScalarType::Unit {
                String::new()
            } else {
                format!(", {} *result_out", c_type(result))
            }
        )
        .unwrap();
    }
    for function in closure {
        let parameters = parameter_facts(function)?;
        let result = scalar_type(&function.return_type).ok_or_else(b111)?;
        let params = c_parameters(&parameters);
        write!(output,"static spxnr_status_v1 spxnr1_f_{}(const spxnr_context_v1 *ctx{}{}{} ){{spxnr_status_v1 status=0;(void)ctx;",full_hash(function.id.as_str()),if params.is_empty(){""}else{", "},params,if result==ScalarType::Unit{String::new()}else{format!(", {} *result_out",c_type(result))}).unwrap();
        for index in 0..parameters.len() {
            write!(output, "(void)arg_{index};").unwrap();
        }
        for (index, (parameter, resolved)) in parameters.iter().zip(&function.params).enumerate() {
            write!(
                output,
                "{} v_{}=arg_{};",
                c_type(parameter.ty),
                full_hash(resolved.id.as_str()),
                index
            )
            .unwrap();
        }
        let mut temporary_count = 0usize;
        let mut lines = CExpressionLineArena::new();
        for requirement in &function.requires {
            lines.clear();
            let value = c_expr(requirement, imports, &mut temporary_count, &mut lines)?;
            output.write_str(lines.as_str()?).unwrap();
            write!(
                output,
                "if(!({value}))return (((uint64_t)65533)<<48)|(((uint64_t)2)<<32)|UINT32_C(1);"
            )
            .unwrap();
        }
        lines.clear();
        let value = c_expr(&function.body, imports, &mut temporary_count, &mut lines)?;
        output.write_str(lines.as_str()?).unwrap();
        if result != ScalarType::Unit {
            write!(
                output,
                "{} v_{}={value};",
                c_type(result),
                full_hash(function.result_id.as_str())
            )
            .unwrap();
        }
        for guarantee in &function.ensures {
            lines.clear();
            let value = c_expr(guarantee, imports, &mut temporary_count, &mut lines)?;
            output.write_str(lines.as_str()?).unwrap();
            write!(
                output,
                "if(!({value}))return (((uint64_t)65533)<<48)|(((uint64_t)2)<<32)|UINT32_C(2);"
            )
            .unwrap();
        }
        if result != ScalarType::Unit {
            write!(
                output,
                "*result_out=v_{};",
                full_hash(function.result_id.as_str())
            )
            .unwrap();
        }
        output.write_str("return status;}\n").unwrap();
    }
    for export in exports {
        let params = c_parameters(&export.parameters);
        write!(output, "spxnr_status_v1 {}(const spxnr_context_v1 *ctx{}{}{} ){{spxnr_status_v1 status=spxnr_validate(ctx);if(status!=0)return status;", export.c_symbol, if params.is_empty(){""}else{", "}, params, if export.result==ScalarType::Unit{String::new()}else{format!(", {} *result_out",c_type(export.result))}).unwrap();
        for import in imports {
            write!(
                output,
                "status=spxnr_validate_{}(ctx);if(status!=0)return status;",
                import.rust_method
            )
            .unwrap();
        }
        if export.result != ScalarType::Unit {
            write!(
                output,
                "if(!result_out||((uintptr_t)result_out%_Alignof({}))!=0)return spxnr_adapter(5);",
                c_type(export.result)
            )
            .unwrap();
        }
        for (index, parameter) in export.parameters.iter().enumerate() {
            if parameter.ty == ScalarType::Bool {
                write!(output, "if(arg_{index}>1)return spxnr_adapter(4);").unwrap();
            }
        }
        output
            .write_str("spxnr_context_v1 local=*ctx;local.call_depth=ctx->call_depth+1;")
            .unwrap();
        write!(
            output,
            "status=spxnr1_f_{}(&local{}{}{});",
            full_hash(&export.id),
            if export.parameters.is_empty() {
                ""
            } else {
                ", "
            },
            (0..export.parameters.len())
                .map(|index| format!("arg_{index}"))
                .collect::<Vec<_>>()
                .join(","),
            if export.result == ScalarType::Unit {
                String::new()
            } else {
                ", result_out".to_owned()
            }
        )
        .unwrap();
        output.write_str("return status;}\n").unwrap();
    }
    Ok(())
}

pub(in crate::implementation) fn capability_digest(capabilities: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CAPABILITIES_DOMAIN);
    for capability in capabilities {
        frame(&mut hasher, capability.as_bytes());
    }
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}
