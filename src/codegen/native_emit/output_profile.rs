//! The closed set of native output profiles and the string-runtime
//! selection each one implies.
//!
//! Relocated verbatim from `native_emit/mod.rs`; profile membership decides
//! which reachability-gated runtime text a translation unit receives.

use crate::hir::ResolvedFunction;

use super::function_uses_strings;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeOutputProfile {
    Legacy,
    /// Private original-source allocation owner; no raw-HIR caller can select it.
    CheckedAllocation,
    /// Private sealed partition source unit.
    SelectedPartition,
    /// Exact rederived native-only final constructor and opaque lease boundary.
    SelectedFinalUtf8,
    OwnedDataProvider,
    OwnedUtf8Provider,
    /// Private checked Bytes body with a bridge-owned invocation reservation.
    ReservedBytesProvider,
    StdoutTranscript,
    /// Single-file command-line program (Text Toolkit v1): an ordinary
    /// `fn main() -> i64` whose result is the process exit status, with argv,
    /// two staged output channels, and `file_read_text` below the invocation
    /// directory, all supplied by the generated process adapter.
    SourceCommand,
    SourceSelectedFileCommand,
    SourceSelectedFileResourceCommand,
    /// Project v28 source command with additive heap-backed String and output
    /// capacities. The v26 SourceCommand projection remains frozen.
    SourceResourceCommand,
    UsefulDataCommand,
    LanguageCommandIo,
    StdinStreamCommandIo,
    StdinStreamExitCommandIo,
    StdinStreamTextCommandIo,
    StdinStreamDataCommandIo,
    LineCommandIo,
    /// Bounded Language Network I/O v1: the line-command input/output
    /// machinery plus the closed TCP operation family and its settlement.
    NetworkCommandIo,
    /// HTTPS Client I/O v1: the line-command input/output machinery plus one
    /// bounded libcurl-backed `https_get` operation.
    HttpsCommandIo,
    /// Filesystem I/O v1: a closed read/write operation pair backed only by
    /// callbacks supplied to the generated runner for this invocation.
    FilesystemCommandIo,
    /// Filesystem I/O v2 extends the callback-only v1 carrier.
    FilesystemCommandIoV2,
    FilesystemCommandIoV3,
    EnvironmentCommandIo,
    ProcessCommandIo,
}

/// Representation and provider carrier support are separate decisions:
/// ordinary and owned-data-provider Strings need length headers but no
/// additional status/Bytes ABI solely because Strings occur.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StringRuntimeSelection {
    pub(super) length_delimited: bool,
    pub(super) provider_carriers: bool,
    pub(super) include_instances: bool,
    pub(super) reserved_bytes: bool,
    pub(super) stream_epochs: bool,
    /// The single-file command profile always carries borrowed text and byte
    /// slices, which its argument and output adapters use.
    pub(super) command_carriers: bool,
    pub(super) resource_strings: bool,
    pub(super) selected_final_utf8: bool,
}

impl StringRuntimeSelection {
    pub(super) const FROZEN: Self = Self {
        length_delimited: false,
        provider_carriers: false,
        include_instances: false,
        reserved_bytes: false,
        stream_epochs: false,
        command_carriers: false,
        resource_strings: false,
        selected_final_utf8: false,
    };
}

impl NativeOutputProfile {
    pub(super) const fn byte_allocator(
        self,
        op: crate::byte_ops::ByteOp,
    ) -> (&'static str, &'static str) {
        match (self, op) {
            (Self::SelectedPartition, crate::byte_ops::ByteOp::Copy) => {
                ("spx_partition_copy", "spx_ctx, ")
            }
            (Self::SelectedPartition, crate::byte_ops::ByteOp::Zeroed) => {
                ("spx_partition_zeroed", "spx_ctx, ")
            }
            (Self::ReservedBytesProvider, crate::byte_ops::ByteOp::Copy) => {
                ("spx_bytes_copy_in", "spx_ctx, ")
            }
            (Self::ReservedBytesProvider, crate::byte_ops::ByteOp::Zeroed) => {
                ("spx_bytes_zeroed_in", "spx_ctx, ")
            }
            (_, crate::byte_ops::ByteOp::Copy) => ("spx_bytes_copy", ""),
            (_, crate::byte_ops::ByteOp::Zeroed) => ("spx_bytes_zeroed", ""),
            _ => unreachable!(),
        }
    }

    pub(super) const fn string_runtime(self) -> StringRuntimeSelection {
        match self {
            Self::SelectedFinalUtf8 => StringRuntimeSelection {
                length_delimited: true,
                include_instances: false,
                selected_final_utf8: true,
                ..StringRuntimeSelection::FROZEN
            },
            Self::Legacy
            | Self::CheckedAllocation
            | Self::SelectedPartition
            | Self::StdoutTranscript
            | Self::OwnedDataProvider => StringRuntimeSelection {
                length_delimited: true,
                provider_carriers: false,
                include_instances: true,
                reserved_bytes: false,
                stream_epochs: false,
                command_carriers: false,
                resource_strings: false,
                selected_final_utf8: false,
            },
            Self::SourceCommand | Self::SourceSelectedFileCommand => StringRuntimeSelection {
                length_delimited: true,
                provider_carriers: false,
                include_instances: true,
                reserved_bytes: false,
                stream_epochs: false,
                command_carriers: true,
                resource_strings: false,
                selected_final_utf8: false,
            },
            Self::SourceResourceCommand | Self::SourceSelectedFileResourceCommand => {
                StringRuntimeSelection {
                    length_delimited: true,
                    provider_carriers: false,
                    include_instances: true,
                    reserved_bytes: false,
                    stream_epochs: false,
                    command_carriers: true,
                    resource_strings: true,
                    selected_final_utf8: false,
                }
            }
            Self::OwnedUtf8Provider => StringRuntimeSelection {
                length_delimited: true,
                provider_carriers: true,
                include_instances: false,
                reserved_bytes: false,
                stream_epochs: false,
                command_carriers: false,
                resource_strings: false,
                selected_final_utf8: false,
            },
            Self::ReservedBytesProvider => StringRuntimeSelection {
                length_delimited: true,
                provider_carriers: true,
                include_instances: false,
                reserved_bytes: true,
                stream_epochs: false,
                command_carriers: false,
                resource_strings: false,
                selected_final_utf8: false,
            },
            Self::StdinStreamTextCommandIo | Self::StdinStreamDataCommandIo => {
                StringRuntimeSelection {
                    length_delimited: true,
                    include_instances: true,
                    stream_epochs: true,
                    command_carriers: true,
                    ..StringRuntimeSelection::FROZEN
                }
            }
            Self::StdinStreamCommandIo | Self::StdinStreamExitCommandIo => StringRuntimeSelection {
                stream_epochs: true,
                command_carriers: true,
                ..StringRuntimeSelection::FROZEN
            },
            Self::UsefulDataCommand
            | Self::LanguageCommandIo
            | Self::LineCommandIo
            | Self::NetworkCommandIo
            | Self::HttpsCommandIo
            | Self::FilesystemCommandIo
            | Self::FilesystemCommandIoV2
            | Self::FilesystemCommandIoV3
            | Self::EnvironmentCommandIo
            | Self::ProcessCommandIo => StringRuntimeSelection::FROZEN,
        }
    }

    pub(super) const fn is_stdin_stream(self) -> bool {
        matches!(
            self,
            Self::StdinStreamCommandIo
                | Self::StdinStreamExitCommandIo
                | Self::StdinStreamTextCommandIo
                | Self::StdinStreamDataCommandIo
        )
    }

    pub(super) const fn tracks_present_strings(self) -> bool {
        matches!(
            self,
            Self::Legacy
                | Self::CheckedAllocation
                | Self::SelectedFinalUtf8
                | Self::StdoutTranscript
                | Self::SourceCommand
                | Self::SourceSelectedFileCommand
                | Self::SourceSelectedFileResourceCommand
                | Self::SourceResourceCommand
                | Self::OwnedDataProvider
                | Self::StdinStreamTextCommandIo
                | Self::StdinStreamDataCommandIo
        )
    }

    pub(super) fn tracks_strings(self, function: &ResolvedFunction) -> bool {
        matches!(self, Self::OwnedUtf8Provider | Self::ReservedBytesProvider)
            || (self.tracks_present_strings() && function_uses_strings(function))
    }

    pub(super) const fn supports_stdout_transcript(self) -> bool {
        matches!(
            self,
            Self::StdoutTranscript
                | Self::SourceCommand
                | Self::SourceSelectedFileCommand
                | Self::SourceSelectedFileResourceCommand
                | Self::SourceResourceCommand
                | Self::UsefulDataCommand
                | Self::LanguageCommandIo
                | Self::StdinStreamCommandIo
                | Self::StdinStreamExitCommandIo
                | Self::StdinStreamTextCommandIo
                | Self::StdinStreamDataCommandIo
                | Self::LineCommandIo
                | Self::NetworkCommandIo
                | Self::HttpsCommandIo
                | Self::EnvironmentCommandIo
                | Self::ProcessCommandIo
        )
    }

    /// Profiles that carry the injected command context instead of a public
    /// `main`: their prelude omits the public failure reporter.
    pub(super) const fn is_command(self) -> bool {
        matches!(
            self,
            Self::UsefulDataCommand
                | Self::LanguageCommandIo
                | Self::StdinStreamCommandIo
                | Self::StdinStreamExitCommandIo
                | Self::StdinStreamTextCommandIo
                | Self::StdinStreamDataCommandIo
                | Self::LineCommandIo
                | Self::NetworkCommandIo
                | Self::HttpsCommandIo
                | Self::FilesystemCommandIo
                | Self::FilesystemCommandIoV2
                | Self::FilesystemCommandIoV3
                | Self::EnvironmentCommandIo
                | Self::ProcessCommandIo
        )
    }

    /// Profiles whose semantic functions see the Language Command I/O v1
    /// context (argument, stdin, and two-channel output carriers).
    pub(super) const fn is_source_resource(self) -> bool {
        matches!(
            self,
            Self::SourceResourceCommand | Self::SourceSelectedFileResourceCommand
        )
    }

    pub(super) const fn is_language_command(self) -> bool {
        matches!(
            self,
            Self::LanguageCommandIo
                | Self::StdinStreamCommandIo
                | Self::StdinStreamExitCommandIo
                | Self::StdinStreamTextCommandIo
                | Self::StdinStreamDataCommandIo
                | Self::LineCommandIo
                | Self::NetworkCommandIo
                | Self::HttpsCommandIo
                | Self::EnvironmentCommandIo
                | Self::ProcessCommandIo
                | Self::SourceCommand
                | Self::SourceSelectedFileCommand
                | Self::SourceSelectedFileResourceCommand
                | Self::SourceResourceCommand
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_resource_selection_is_additive() {
        let frozen = NativeOutputProfile::SourceCommand.string_runtime();
        let resource = NativeOutputProfile::SourceResourceCommand.string_runtime();
        assert!(!frozen.resource_strings);
        assert!(resource.resource_strings);
        assert_eq!(
            StringRuntimeSelection {
                resource_strings: false,
                selected_final_utf8: false,
                ..resource
            },
            frozen
        );
        assert!(NativeOutputProfile::SourceResourceCommand.is_language_command());
        assert!(!NativeOutputProfile::SourceResourceCommand.is_command());
    }

    #[test]
    fn stream_text_selection_is_additive_and_old_string_runtime_is_frozen() {
        for profile in [
            NativeOutputProfile::StdinStreamCommandIo,
            NativeOutputProfile::StdinStreamExitCommandIo,
        ] {
            assert_eq!(
                profile.string_runtime(),
                StringRuntimeSelection {
                    stream_epochs: true,
                    command_carriers: true,
                    ..StringRuntimeSelection::FROZEN
                }
            );
            assert!(!profile.tracks_present_strings());
        }
        let selected = NativeOutputProfile::StdinStreamTextCommandIo;
        assert_eq!(
            selected.string_runtime(),
            StringRuntimeSelection {
                length_delimited: true,
                include_instances: true,
                stream_epochs: true,
                command_carriers: true,
                ..StringRuntimeSelection::FROZEN
            }
        );
        assert!(
            selected.tracks_present_strings()
                && selected.is_command()
                && selected.is_stdin_stream()
        );
        assert_eq!(
            NativeOutputProfile::StdinStreamDataCommandIo.string_runtime(),
            selected.string_runtime()
        );
    }
}
