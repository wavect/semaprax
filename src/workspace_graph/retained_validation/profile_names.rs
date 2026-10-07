//! Names of the independently admitted Project linkers.

pub(in crate::workspace_graph) fn project_linker_name(
    profile: crate::project::ProjectProfile,
) -> &'static str {
    use crate::project::ProjectProfile as P;
    match profile {
        P::EnvironmentIoV1 => "Environment I/O v1 linker",
        P::ProcessIoV1 => "Process I/O v1 linker",
        P::FilesystemIoV1 | P::FilesystemIoV2 | P::FilesystemIoV3 => "Filesystem I/O v1 linker",
        P::ScalarV1 => "pure scalar linker",
        P::UsefulTextConsumerV1 => "Useful Text Consumer linker",
        P::UsefulDataV1 | P::UsefulDataV2 => "Useful Data linker",
        P::UsefulDataCommandV1 => "Useful Data Command linker",
        P::UsefulDataCommandV2 => "Useful Data Command v2 linker",
        P::LanguageCommandIoV1 => "Language Command I/O v1 linker",
        P::StdinStreamCommandIoV1 => "Streaming Command I/O v1 linker",
        P::LineCommandIoV1 => "Line Command I/O v1 linker",
        P::NetworkCommandIoV1 => "Network Command I/O v1 linker",
        P::HttpsCommandIoV1 => "HTTPS Command I/O v1 linker",
        P::OwnedDataApiV1 => "Owned Data API v1 linker",
        P::FlatOwnedRecordApiV1 => "Flat Owned Record API v1 linker",
        P::OwnedUtf8ApiV1 => "Owned UTF-8 API v1 linker",
        P::NestedOwnedRecordApiV1 => "Nested Owned Record API v1 linker",
        P::PublicGenericWasmProviderV1 => "Public Generic Wasm Provider v1 linker",
        P::SourceLocalFutureV1 => "Source Local Future v1 linker",
        P::SourceLocalFutureIndexedRustV1 => "Source Local Future indexed Rust v1 linker",
    }
}
