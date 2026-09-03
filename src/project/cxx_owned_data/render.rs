use std::fmt::Write;

use super::super::{PublicApiDescriptor, PublicApiParameterType, PublicApiResultType};
use crate::bounded_output::CappedString;

pub(super) fn c_header(descriptor: &PublicApiDescriptor) -> String {
    let mut out = CappedString::new();
    out.push_str("#ifndef SEMAPRAX_OWNED_DATA_V1_H\n#define SEMAPRAX_OWNED_DATA_V1_H\n#include <stdint.h>\n#ifdef __cplusplus\nextern \"C\" {\n#endif\ntypedef uint32_t spx_owned_data_status_v1;\ntypedef uint64_t spx_owned_bytes_handle_v1;\ntypedef struct spx_owned_data_context_v1 spx_context_v1;\nenum { SPX_OWNED_DATA_SUCCESS=0, SPX_OWNED_DATA_SEMANTIC_FAILURE=1, SPX_OWNED_DATA_ADAPTER_FAILURE=2, SPX_OWNED_DATA_INVALID_HANDLE=3, SPX_OWNED_DATA_COPY_FAILURE=4, SPX_OWNED_DATA_SETTLEMENT_FAILURE=5 };\nuint64_t spx_owned_data_context_size_v1(void);\nuint64_t spx_owned_data_context_align_v1(void);\nspx_owned_data_status_v1 spx_owned_data_context_init_v1(void*,uint64_t);\nspx_owned_data_status_v1 spx_owned_data_context_drop_v1(spx_context_v1*);\nspx_owned_data_status_v1 spx_owned_bytes_len_v1(spx_context_v1*,spx_owned_bytes_handle_v1,uint64_t*);\nspx_owned_data_status_v1 spx_owned_bytes_copy_v1(spx_context_v1*,spx_owned_bytes_handle_v1,uint8_t*,uint64_t);\nspx_owned_data_status_v1 spx_owned_bytes_drop_v1(spx_context_v1*,spx_owned_bytes_handle_v1);\n");
    for export in descriptor.exports() {
        write!(
            out,
            "spx_owned_data_status_v1 spx_owned_data_call_{}_v1(spx_context_v1*",
            export.rust_method_name()
        )
        .unwrap();
        for parameter in export.parameters() {
            match parameter.ty() {
                PublicApiParameterType::I64 => out.push_str(",int64_t"),
                PublicApiParameterType::Bool => out.push_str(",uint8_t"),
                PublicApiParameterType::BorrowStr | PublicApiParameterType::BorrowSliceU8 => {
                    out.push_str(",const uint8_t*,uint64_t")
                }
            }
        }
        match export.result() {
            PublicApiResultType::I64 => out.push_str(",int64_t*);\n"),
            PublicApiResultType::Bool => out.push_str(",uint8_t*);\n"),
            PublicApiResultType::Usize => out.push_str(",uint64_t*);\n"),
            _ => out.push_str(",uint32_t*,spx_owned_bytes_handle_v1*,int64_t*);\n"),
        }
    }
    out.push_str("#ifdef __cplusplus\n}\n#endif\n#endif\n");
    out.into_string()
}

pub(super) fn cxx_header(descriptor: &PublicApiDescriptor) -> String {
    let mut out = CappedString::new();
    out.push_str("#ifndef SEMAPRAX_OWNED_DATA_V1_HPP\n#define SEMAPRAX_OWNED_DATA_V1_HPP\n#include \"semaprax_owned_data.h\"\n#include <cstddef>\n#include <cstdint>\n#include <exception>\n#include <limits>\n#include <new>\n#include <optional>\n#include <stdexcept>\n#include <string_view>\n#include <thread>\n#include <utility>\n#include <variant>\n#include <vector>\nnamespace semaprax::owned_data_v1 {\nusing Bytes=std::vector<std::uint8_t>;\nusing BytesResult=std::variant<Bytes,std::int64_t>;\nstruct ByteView { const std::uint8_t* data; std::uint64_t size; ByteView(const std::uint8_t* p,std::uint64_t n):data(p),size(n){if(n&&p==nullptr)throw std::invalid_argument(\"null byte view\");} };\nclass Failure final:public std::runtime_error{public:explicit Failure(spx_owned_data_status_v1 s):std::runtime_error(\"Semaprax invocation failed\"),status_(s){} spx_owned_data_status_v1 status()const noexcept{return status_;}private:spx_owned_data_status_v1 status_;};\nclass Client final {\n public:\n  Client():owner_(std::this_thread::get_id()){size_=spx_owned_data_context_size_v1();align_=spx_owned_data_context_align_v1();if(!size_||size_>(UINT64_C(1)<<20)||!align_||(align_&(align_-1))||align_>size_)std::terminate();storage_=::operator new(static_cast<std::size_t>(size_),std::align_val_t(static_cast<std::size_t>(align_)));}\n  ~Client() noexcept{if(std::this_thread::get_id()!=owner_||active_)std::terminate();::operator delete(storage_,std::align_val_t(static_cast<std::size_t>(align_)));}\n  Client(const Client&)=delete;Client& operator=(const Client&)=delete;Client(Client&&)=delete;Client& operator=(Client&&)=delete;\n");
    for export in descriptor.exports() {
        emit_method(&mut out, export);
    }
    out.push_str(
        " private:\n  void begin(){if(std::this_thread::get_id()!=owner_||active_||spx_owned_data_context_init_v1(storage_,size_)!=SPX_OWNED_DATA_SUCCESS)std::terminate();active_=true;}\n  void close(){if(!active_||spx_owned_data_context_drop_v1(context())!=SPX_OWNED_DATA_SUCCESS)std::terminate();active_=false;}\n  [[noreturn]] void fail(spx_owned_data_status_v1 s){close();if(s==SPX_OWNED_DATA_SEMANTIC_FAILURE||s==SPX_OWNED_DATA_ADAPTER_FAILURE)throw Failure(s);std::terminate();}\n  Bytes take(spx_owned_bytes_handle_v1 h){if(!h)std::terminate();std::uint64_t n=UINT64_MAX;if(spx_owned_bytes_len_v1(context(),h,&n)!=SPX_OWNED_DATA_SUCCESS||n>UINT64_C(65536))std::terminate();Bytes value;try{value.resize(static_cast<std::size_t>(n));}catch(...){if(spx_owned_bytes_drop_v1(context(),h)!=SPX_OWNED_DATA_SUCCESS)std::terminate();close();throw;}if(spx_owned_bytes_copy_v1(context(),h,value.data(),n)!=SPX_OWNED_DATA_SUCCESS){if(spx_owned_bytes_drop_v1(context(),h)!=SPX_OWNED_DATA_SUCCESS)std::terminate();close();throw Failure(SPX_OWNED_DATA_COPY_FAILURE);}if(spx_owned_bytes_drop_v1(context(),h)!=SPX_OWNED_DATA_SUCCESS)std::terminate();return value;}\n  spx_context_v1* context()noexcept{return static_cast<spx_context_v1*>(storage_);}\n  static void add_input(std::uint64_t& total,std::uint64_t n){if(n>UINT64_C(65536)-total)throw std::length_error(\"borrowed input limit\");total+=n;}\n  void* storage_=nullptr;std::uint64_t size_=0,align_=0;bool active_=false;std::thread::id owner_;\n};\n}\n#endif\n",
    );
    out.into_string()
}

fn emit_method(out: &mut CappedString, export: &super::super::PublicApiExport) {
    let return_type = match export.result() {
        PublicApiResultType::I64 => "std::int64_t",
        PublicApiResultType::Bool => "bool",
        PublicApiResultType::Usize => "std::uint64_t",
        PublicApiResultType::OwnedBytes => "Bytes",
        PublicApiResultType::OptionOwnedBytes => "std::optional<Bytes>",
        PublicApiResultType::ResultOwnedBytesI64 => "BytesResult",
        PublicApiResultType::OwnedUtf8 => unreachable!("v10 is rejected before rendering"),
    };
    write!(out, "  {} {}(", return_type, export.rust_method_name()).unwrap();
    for (index, parameter) in export.parameters().iter().enumerate() {
        if index != 0 {
            out.push(',');
        }
        let ty = match parameter.ty() {
            PublicApiParameterType::I64 => "std::int64_t",
            PublicApiParameterType::Bool => "bool",
            PublicApiParameterType::BorrowStr => "std::string_view",
            PublicApiParameterType::BorrowSliceU8 => "ByteView",
        };
        write!(out, "{} a{}", ty, index).unwrap();
    }
    out.push_str("){std::uint64_t borrowed=0;");
    for (index, parameter) in export.parameters().iter().enumerate() {
        if matches!(
            parameter.ty(),
            PublicApiParameterType::BorrowStr | PublicApiParameterType::BorrowSliceU8
        ) {
            write!(
                out,
                "add_input(borrowed,static_cast<std::uint64_t>(a{}.size{}));",
                index,
                if parameter.ty() == PublicApiParameterType::BorrowStr {
                    "()"
                } else {
                    ""
                }
            )
            .unwrap();
        }
    }
    out.push_str("(void)borrowed;");
    out.push_str("begin();");
    let owned = !matches!(
        export.result(),
        PublicApiResultType::I64 | PublicApiResultType::Bool | PublicApiResultType::Usize
    );
    if owned {
        out.push_str("std::uint32_t tag=UINT32_MAX;spx_owned_bytes_handle_v1 handle=0;std::int64_t error=INT64_MIN;");
    } else {
        match export.result() {
            PublicApiResultType::I64 => out.push_str("std::int64_t value=INT64_MIN;"),
            PublicApiResultType::Bool => out.push_str("std::uint8_t value=UINT8_MAX;"),
            _ => out.push_str("std::uint64_t value=UINT64_MAX;"),
        }
    }
    write!(
        out,
        "auto status=spx_owned_data_call_{}_v1(context()",
        export.rust_method_name()
    )
    .unwrap();
    for (index, parameter) in export.parameters().iter().enumerate() {
        match parameter.ty() { PublicApiParameterType::I64=>write!(out,",a{}",index), PublicApiParameterType::Bool=>write!(out,",a{}?UINT8_C(1):UINT8_C(0)",index), PublicApiParameterType::BorrowStr=>write!(out,",reinterpret_cast<const std::uint8_t*>(a{}.data()),static_cast<std::uint64_t>(a{}.size())",index,index), PublicApiParameterType::BorrowSliceU8=>write!(out,",a{}.data,a{}.size",index,index) }.unwrap();
    }
    if owned {
        out.push_str(",&tag,&handle,&error);");
    } else {
        out.push_str(",&value);");
    }
    out.push_str("if(status!=SPX_OWNED_DATA_SUCCESS){");
    if owned {
        out.push_str("if(tag!=UINT32_MAX||handle!=0||error!=INT64_MIN)std::terminate();");
    } else {
        match export.result() {
            PublicApiResultType::I64 => out.push_str("if(value!=INT64_MIN)std::terminate();"),
            PublicApiResultType::Bool => out.push_str("if(value!=UINT8_MAX)std::terminate();"),
            _ => out.push_str("if(value!=UINT64_MAX)std::terminate();"),
        }
    }
    out.push_str("fail(status);}");
    match export.result() {
        PublicApiResultType::I64 => out.push_str("close();return value;}\n"),
        PublicApiResultType::Bool => out.push_str("if(value>1)std::terminate();close();return value!=0;}\n"),
        PublicApiResultType::Usize => out.push_str("close();return value;}\n"),
        PublicApiResultType::OwnedBytes => out.push_str("if(tag!=0||!handle||error!=0)std::terminate();auto result=take(handle);close();return result;}\n"),
        PublicApiResultType::OptionOwnedBytes => out.push_str("if(tag==0){if(handle||error)std::terminate();close();return std::nullopt;}if(tag!=1||!handle||error)std::terminate();auto result=take(handle);close();return result;}\n"),
        PublicApiResultType::ResultOwnedBytesI64 => out.push_str("if(tag==1){if(handle)std::terminate();close();return BytesResult(std::in_place_index<1>,error);}if(tag!=0||!handle||error)std::terminate();auto result=take(handle);close();return BytesResult(std::in_place_index<0>,std::move(result));}\n"),
        PublicApiResultType::OwnedUtf8 => unreachable!(),
    }
}
