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
    OwnedDataProvider,
    OwnedUtf8Provider,
    StdoutTranscript,
    UsefulDataCommand,
    LanguageCommandIo,
    LineCommandIo,
    /// Bounded Language Network I/O v1: the line-command input/output
    /// machinery plus the closed TCP operation family and its settlement.
    NetworkCommandIo,
    /// HTTPS Client I/O v1: the line-command input/output machinery plus one
    /// bounded libcurl-backed `https_get` operation.
    HttpsCommandIo,
}

/// Representation and provider carrier support are separate decisions:
/// ordinary and owned-data-provider Strings need length headers but no
/// additional status/Bytes ABI solely because Strings occur.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StringRuntimeSelection {
    pub(super) length_delimited: bool,
    pub(super) provider_carriers: bool,
    pub(super) include_instances: bool,
}

impl StringRuntimeSelection {
    pub(super) const FROZEN: Self = Self {
        length_delimited: false,
        provider_carriers: false,
        include_instances: false,
    };
}

impl NativeOutputProfile {
    pub(super) const fn string_runtime(self) -> StringRuntimeSelection {
        match self {
            Self::Legacy | Self::StdoutTranscript | Self::OwnedDataProvider => {
                StringRuntimeSelection {
                    length_delimited: true,
                    provider_carriers: false,
                    include_instances: true,
                }
            }
            Self::OwnedUtf8Provider => StringRuntimeSelection {
                length_delimited: true,
                provider_carriers: true,
                include_instances: false,
            },
            Self::UsefulDataCommand
            | Self::LanguageCommandIo
            | Self::LineCommandIo
            | Self::NetworkCommandIo
            | Self::HttpsCommandIo => StringRuntimeSelection::FROZEN,
        }
    }

    pub(super) const fn tracks_present_strings(self) -> bool {
        matches!(
            self,
            Self::Legacy | Self::StdoutTranscript | Self::OwnedDataProvider
        )
    }

    pub(super) fn tracks_strings(self, function: &ResolvedFunction) -> bool {
        self == Self::OwnedUtf8Provider
            || (self.tracks_present_strings() && function_uses_strings(function))
    }

    pub(super) const fn supports_stdout_transcript(self) -> bool {
        matches!(
            self,
            Self::StdoutTranscript
                | Self::UsefulDataCommand
                | Self::LanguageCommandIo
                | Self::LineCommandIo
                | Self::NetworkCommandIo
                | Self::HttpsCommandIo
        )
    }

    /// Profiles that carry the injected command context instead of a public
    /// `main`: their prelude omits the public failure reporter.
    pub(super) const fn is_command(self) -> bool {
        matches!(
            self,
            Self::UsefulDataCommand
                | Self::LanguageCommandIo
                | Self::LineCommandIo
                | Self::NetworkCommandIo
                | Self::HttpsCommandIo
        )
    }

    /// Profiles whose semantic functions see the Language Command I/O v1
    /// context (argument, stdin, and two-channel output carriers).
    pub(super) const fn is_language_command(self) -> bool {
        matches!(
            self,
            Self::LanguageCommandIo
                | Self::LineCommandIo
                | Self::NetworkCommandIo
                | Self::HttpsCommandIo
        )
    }
}
