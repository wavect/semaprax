use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::diagnostic::quote_json;

use super::{FlatOwnedRecordApiDescriptor, FlatOwnedRecordExport, PublicApiParameterType};

/// Render the low-level C11 boundary implemented by the authenticated native
/// provider. Record results use descriptor-order `uint64_t` carrier slots;
/// owned byte slots are opaque handles and remain provider-owned until dropped.
pub fn render_flat_owned_record_c_header(descriptor: &FlatOwnedRecordApiDescriptor) -> String {
    let mut output = String::from(
        "#ifndef SEMAPRAX_FLAT_OWNED_RECORD_V1_H\n#define SEMAPRAX_FLAT_OWNED_RECORD_V1_H\n#include <stdint.h>\n#ifdef __cplusplus\n#define SPX_FLAT_RECORD_STATIC(N) N\nextern \"C\" {\n#else\n#define SPX_FLAT_RECORD_STATIC(N) static N\n#endif\ntypedef uint32_t spx_owned_data_status_v1;\ntypedef uint64_t spx_owned_bytes_handle_v1;\ntypedef struct spx_owned_data_context_v1 spx_context_v1;\nenum { SPX_OWNED_DATA_SUCCESS=0, SPX_OWNED_DATA_SEMANTIC_FAILURE=1, SPX_OWNED_DATA_ADAPTER_FAILURE=2, SPX_OWNED_DATA_INVALID_HANDLE=3, SPX_OWNED_DATA_COPY_FAILURE=4, SPX_OWNED_DATA_SETTLEMENT_FAILURE=5 };\nenum { SPX_FLAT_RECORD_I64=0, SPX_FLAT_RECORD_BOOL=1, SPX_FLAT_RECORD_USIZE=2, SPX_FLAT_RECORD_OWNED_BYTES=3 };\nuint64_t spx_owned_data_context_size_v1(void);\nuint64_t spx_owned_data_context_align_v1(void);\nspx_owned_data_status_v1 spx_owned_data_context_init_v1(void*,uint64_t);\nspx_owned_data_status_v1 spx_owned_data_context_drop_v1(spx_context_v1*);\nspx_owned_data_status_v1 spx_owned_bytes_len_v1(spx_context_v1*,spx_owned_bytes_handle_v1,uint64_t*);\nspx_owned_data_status_v1 spx_owned_bytes_copy_v1(spx_context_v1*,spx_owned_bytes_handle_v1,uint8_t*,uint64_t);\nspx_owned_data_status_v1 spx_owned_bytes_drop_v1(spx_context_v1*,spx_owned_bytes_handle_v1);\n",
    );
    for (export_index, export) in descriptor.exports.iter().enumerate() {
        writeln!(
            output,
            "#define SPX_FLAT_RECORD_EXPORT_{export_index}_FIELD_COUNT UINT32_C({})",
            export.fields.len()
        )
        .unwrap();
        for (field_index, field) in export.fields.iter().enumerate() {
            debug_assert_eq!(field_index, field.ordinal as usize);
            writeln!(
                output,
                "#define SPX_FLAT_RECORD_EXPORT_{export_index}_FIELD_{field_index} UINT32_C({field_index})"
            )
            .unwrap();
            let kind = match field.ty {
                super::FlatOwnedRecordFieldType::I64 => "SPX_FLAT_RECORD_I64",
                super::FlatOwnedRecordFieldType::Bool => "SPX_FLAT_RECORD_BOOL",
                super::FlatOwnedRecordFieldType::Usize => "SPX_FLAT_RECORD_USIZE",
                super::FlatOwnedRecordFieldType::OwnedBytes => "SPX_FLAT_RECORD_OWNED_BYTES",
            };
            writeln!(
                output,
                "#define SPX_FLAT_RECORD_EXPORT_{export_index}_FIELD_{field_index}_KIND {kind}"
            )
            .unwrap();
        }
        write!(
            output,
            "spx_owned_data_status_v1 spx_owned_data_call_{}_v1(spx_context_v1*",
            export.rust_method_name
        )
        .unwrap();
        for (_, _, parameter) in &export.parameters {
            match parameter {
                PublicApiParameterType::I64 => output.push_str(",int64_t"),
                PublicApiParameterType::Bool => output.push_str(",uint8_t"),
                PublicApiParameterType::BorrowStr | PublicApiParameterType::BorrowSliceU8 => {
                    output.push_str(",const uint8_t*,uint64_t")
                }
            }
        }
        writeln!(
            output,
            ",uint64_t[SPX_FLAT_RECORD_STATIC({})]);",
            export.fields.len()
        )
        .unwrap();
    }
    output.push_str("#ifdef __cplusplus\n}\n#endif\n#undef SPX_FLAT_RECORD_STATIC\n#endif\n");
    output
}

/// Render the safe C++17 value adapter over the low-level C boundary.
/// Opaque byte handles are copied and settled before a result is published.
pub fn render_flat_owned_record_cpp_header(descriptor: &FlatOwnedRecordApiDescriptor) -> String {
    let mut records = BTreeMap::<&str, &FlatOwnedRecordExport>::new();
    for export in &descriptor.exports {
        records.entry(&export.record_host_name).or_insert(export);
    }
    let mut output = String::from(
        "#ifndef SEMAPRAX_FLAT_OWNED_RECORD_V1_HPP\n#define SEMAPRAX_FLAT_OWNED_RECORD_V1_HPP\n#include \"semaprax_flat_owned_record.h\"\n#include <cstddef>\n#include <cstdint>\n#include <cstring>\n#include <exception>\n#include <limits>\n#include <new>\n#include <stdexcept>\n#include <string_view>\n#include <thread>\n#include <utility>\n#include <vector>\nnamespace semaprax::flat_owned_record_v1 {\nusing Bytes=std::vector<std::uint8_t>;\nstruct ByteView { const std::uint8_t* data; std::uint64_t size; ByteView(const std::uint8_t* p,std::uint64_t n):data(p),size(n){if(n&&p==nullptr)throw std::invalid_argument(\"null byte view\");} };\nclass Failure final:public std::runtime_error{public:explicit Failure(spx_owned_data_status_v1 s):std::runtime_error(\"Semaprax invocation failed\"),status_(s){} spx_owned_data_status_v1 status()const noexcept{return status_;}private:spx_owned_data_status_v1 status_;};\n",
    );
    for (name, export) in records {
        write!(output, "struct {name} {{").unwrap();
        for field in &export.fields {
            write!(output, "{} {};", cpp_field_type(field.ty), field.host_name).unwrap();
        }
        output.push_str("};\n");
    }
    output.push_str("class Client final {\n public:\n  Client():owner_(std::this_thread::get_id()){size_=spx_owned_data_context_size_v1();align_=spx_owned_data_context_align_v1();if(!size_||size_>(UINT64_C(1)<<20)||!align_||(align_&(align_-1))||align_>size_)std::terminate();storage_=::operator new(static_cast<std::size_t>(size_),std::align_val_t(static_cast<std::size_t>(align_)));}\n  ~Client() noexcept{if(std::this_thread::get_id()!=owner_||active_)std::terminate();::operator delete(storage_,std::align_val_t(static_cast<std::size_t>(align_)));}\n  Client(const Client&)=delete;Client& operator=(const Client&)=delete;Client(Client&&)=delete;Client& operator=(Client&&)=delete;\n");
    for export in &descriptor.exports {
        emit_cpp_method(&mut output, export);
    }
    output.push_str(" private:\n  void begin(){if(std::this_thread::get_id()!=owner_||active_||spx_owned_data_context_init_v1(storage_,size_)!=SPX_OWNED_DATA_SUCCESS)std::terminate();active_=true;}\n  void close(){if(!active_||spx_owned_data_context_drop_v1(context())!=SPX_OWNED_DATA_SUCCESS)std::terminate();active_=false;}\n  [[noreturn]] void fail(spx_owned_data_status_v1 s){close();if(s==SPX_OWNED_DATA_SEMANTIC_FAILURE||s==SPX_OWNED_DATA_ADAPTER_FAILURE)throw Failure(s);std::terminate();}\n  Bytes take(spx_owned_bytes_handle_v1 h){if(!h)std::terminate();std::uint64_t n=UINT64_MAX;if(spx_owned_bytes_len_v1(context(),h,&n)!=SPX_OWNED_DATA_SUCCESS)std::terminate();if(n>UINT64_C(65536)){if(spx_owned_bytes_drop_v1(context(),h)!=SPX_OWNED_DATA_SUCCESS)std::terminate();close();throw Failure(SPX_OWNED_DATA_ADAPTER_FAILURE);}Bytes value;try{value.resize(static_cast<std::size_t>(n));}catch(...){if(spx_owned_bytes_drop_v1(context(),h)!=SPX_OWNED_DATA_SUCCESS)std::terminate();close();throw;}if(spx_owned_bytes_copy_v1(context(),h,value.data(),n)!=SPX_OWNED_DATA_SUCCESS){if(spx_owned_bytes_drop_v1(context(),h)!=SPX_OWNED_DATA_SUCCESS)std::terminate();close();throw Failure(SPX_OWNED_DATA_COPY_FAILURE);}if(spx_owned_bytes_drop_v1(context(),h)!=SPX_OWNED_DATA_SUCCESS)std::terminate();return value;}\n  spx_context_v1* context()noexcept{return static_cast<spx_context_v1*>(storage_);}\n  static std::int64_t decode_i64(std::uint64_t bits)noexcept{std::int64_t value=0;std::memcpy(&value,&bits,sizeof(value));return value;}\n  static void add_input(std::uint64_t& total,std::uint64_t n){if(n>UINT64_C(65536)-total)throw std::length_error(\"borrowed input limit\");total+=n;}\n  void* storage_=nullptr;std::uint64_t size_=0,align_=0;bool active_=false;std::thread::id owner_;\n};\n}\n#endif\n");
    output
}

fn emit_cpp_method(output: &mut String, export: &FlatOwnedRecordExport) {
    write!(
        output,
        "  {} {}(",
        export.record_host_name, export.rust_method_name
    )
    .unwrap();
    for (index, (_, _, parameter)) in export.parameters.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write!(output, "{} a{index}", cpp_parameter_type(*parameter)).unwrap();
    }
    output.push_str("){std::uint64_t borrowed=0;");
    for (index, (_, _, parameter)) in export.parameters.iter().enumerate() {
        match parameter {
            PublicApiParameterType::BorrowStr => write!(
                output,
                "add_input(borrowed,static_cast<std::uint64_t>(a{index}.size()));"
            )
            .unwrap(),
            PublicApiParameterType::BorrowSliceU8 => {
                write!(output, "add_input(borrowed,a{index}.size);").unwrap()
            }
            _ => {}
        }
    }
    output.push_str("(void)borrowed;begin();std::uint64_t carrier[");
    write!(output, "{}", export.fields.len()).unwrap();
    output.push_str("];for(auto& slot:carrier)slot=UINT64_MAX;auto status=spx_owned_data_call_");
    output.push_str(&export.rust_method_name);
    output.push_str("_v1(context()");
    for (index, (_, _, parameter)) in export.parameters.iter().enumerate() {
        match parameter {
            PublicApiParameterType::I64 => write!(output, ",a{index}").unwrap(),
            PublicApiParameterType::Bool => write!(output, ",a{index}?UINT8_C(1):UINT8_C(0)").unwrap(),
            PublicApiParameterType::BorrowStr => write!(output, ",reinterpret_cast<const std::uint8_t*>(a{index}.data()),static_cast<std::uint64_t>(a{index}.size())").unwrap(),
            PublicApiParameterType::BorrowSliceU8 => write!(output, ",a{index}.data,a{index}.size").unwrap(),
        }
    }
    output.push_str(",carrier);if(status!=SPX_OWNED_DATA_SUCCESS){for(auto slot:carrier)if(slot!=UINT64_MAX)std::terminate();fail(status);}");
    let byte_field = export
        .fields
        .iter()
        .find(|field| field.ty == super::FlatOwnedRecordFieldType::OwnedBytes)
        .expect("v9 admission has one byte field");
    for field in &export.fields {
        if field.ty == super::FlatOwnedRecordFieldType::Bool {
            write!(
                output,
                "if(carrier[{}]>UINT64_C(1))std::terminate();",
                field.ordinal
            )
            .unwrap();
        }
    }
    write!(
        output,
        "auto owned=take(carrier[{}]);close();return {}{{",
        byte_field.ordinal, export.record_host_name
    )
    .unwrap();
    for (index, field) in export.fields.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        match field.ty {
            super::FlatOwnedRecordFieldType::I64 => {
                write!(output, "decode_i64(carrier[{}])", field.ordinal).unwrap()
            }
            super::FlatOwnedRecordFieldType::Bool => {
                write!(output, "carrier[{}]!=0", field.ordinal).unwrap()
            }
            super::FlatOwnedRecordFieldType::Usize => {
                write!(output, "carrier[{}]", field.ordinal).unwrap()
            }
            super::FlatOwnedRecordFieldType::OwnedBytes => output.push_str("std::move(owned)"),
        }
    }
    output.push_str("};}\n");
}

fn cpp_field_type(ty: super::FlatOwnedRecordFieldType) -> &'static str {
    match ty {
        super::FlatOwnedRecordFieldType::I64 => "std::int64_t",
        super::FlatOwnedRecordFieldType::Bool => "bool",
        super::FlatOwnedRecordFieldType::Usize => "std::uint64_t",
        super::FlatOwnedRecordFieldType::OwnedBytes => "Bytes",
    }
}

fn cpp_parameter_type(ty: PublicApiParameterType) -> &'static str {
    match ty {
        PublicApiParameterType::I64 => "std::int64_t",
        PublicApiParameterType::Bool => "bool",
        PublicApiParameterType::BorrowStr => "std::string_view",
        PublicApiParameterType::BorrowSliceU8 => "ByteView",
    }
}

pub fn render_flat_owned_record_typescript(descriptor: &FlatOwnedRecordApiDescriptor) -> String {
    let mut records = BTreeMap::<&str, &FlatOwnedRecordExport>::new();
    for export in &descriptor.exports {
        records.entry(&export.record_host_name).or_insert(export);
    }
    let mut output = String::new();
    for (name, export) in records {
        output.push_str("export interface ");
        output.push_str(name);
        output.push_str(" {\n");
        for field in &export.fields {
            output.push_str("  readonly ");
            output.push_str(&field.host_name);
            output.push_str(": ");
            output.push_str(field.ty.typescript());
            output.push_str(";\n");
        }
        output.push_str("}\n");
    }
    output.push_str("export interface SemapraxApi {\n");
    for export in &descriptor.exports {
        output.push_str("  readonly ");
        output.push_str(&quote_json(export.typescript_name()));
        output.push_str(": (");
        for (index, (_, _, ty)) in export.parameters.iter().enumerate() {
            if index != 0 {
                output.push_str(", ");
            }
            output.push_str("arg");
            output.push_str(&index.to_string());
            output.push_str(": ");
            output.push_str(parameter_typescript(*ty));
        }
        output.push_str(") => ");
        output.push_str(&export.record_host_name);
        output.push_str(";\n");
    }
    output.push_str("}\n");
    output
}

pub fn render_flat_owned_record_rust(descriptor: &FlatOwnedRecordApiDescriptor) -> String {
    let mut records = BTreeMap::<&str, &FlatOwnedRecordExport>::new();
    for export in &descriptor.exports {
        records.entry(&export.record_host_name).or_insert(export);
    }
    let mut output = String::from("#![forbid(unsafe_code)]\n#[derive(Clone, Debug, Eq, PartialEq)]\npub struct CallError { message: &'static str }\nimpl CallError { pub fn message(&self) -> &str { self.message } }\n");
    for (name, export) in records {
        output.push_str("#[derive(Clone, Debug, Eq, PartialEq)]\npub struct ");
        output.push_str(name);
        output.push_str(" {\n");
        for field in &export.fields {
            output.push_str("    pub ");
            output.push_str(&field.host_name);
            output.push_str(": ");
            output.push_str(field.ty.rust());
            output.push_str(",\n");
        }
        output.push_str("}\n");
    }
    output.push_str("pub trait SemapraxApi {\n");
    for export in &descriptor.exports {
        output.push_str("    fn ");
        output.push_str(export.rust_method_name());
        output.push_str("(&self");
        for (index, (_, _, ty)) in export.parameters.iter().enumerate() {
            output.push_str(", arg");
            output.push_str(&index.to_string());
            output.push_str(": ");
            output.push_str(parameter_rust(*ty));
        }
        output.push_str(") -> Result<");
        output.push_str(&export.record_host_name);
        output.push_str(", CallError>;\n");
    }
    output.push_str("}\n");
    output
}

fn parameter_typescript(ty: PublicApiParameterType) -> &'static str {
    match ty {
        PublicApiParameterType::I64 => "bigint",
        PublicApiParameterType::Bool => "boolean",
        PublicApiParameterType::BorrowStr => "string",
        PublicApiParameterType::BorrowSliceU8 => "Uint8Array",
    }
}

fn parameter_rust(ty: PublicApiParameterType) -> &'static str {
    match ty {
        PublicApiParameterType::I64 => "i64",
        PublicApiParameterType::Bool => "bool",
        PublicApiParameterType::BorrowStr => "&str",
        PublicApiParameterType::BorrowSliceU8 => "&[u8]",
    }
}
