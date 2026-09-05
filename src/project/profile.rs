/// Additive Project profile names. The manifest schema selects exactly one
/// profile; downstream consumers receive this closed enum rather than infer
/// authority from a schema string or boolean flag.
pub const PROJECT_PROFILE_USEFUL_TEXT_CONSUMER_V1: &str = "useful-text-consumer.v1";
pub const PROJECT_PROFILE_USEFUL_DATA_V1: &str = "useful-data.v1";
pub const PROJECT_PROFILE_USEFUL_DATA_COMMAND_V1: &str = "useful-data-command.v1";
pub const PROJECT_PROFILE_USEFUL_DATA_COMMAND_V2: &str = "useful-data-command.v2";
pub const PROJECT_PROFILE_LANGUAGE_COMMAND_IO_V1: &str = "language-command-io.v1";
pub const PROJECT_PROFILE_LINE_COMMAND_IO_V1: &str = "line-command-io.v1";
pub const PROJECT_PROFILE_NETWORK_COMMAND_IO_V1: &str = "network-command-io.v1";
pub const PROJECT_PROFILE_HTTPS_COMMAND_IO_V1: &str = "https-command-io.v1";
pub const PROJECT_PROFILE_OWNED_DATA_API_V1: &str = "owned-data-api.v1";
pub const PROJECT_PROFILE_FLAT_OWNED_RECORD_API_V1: &str = "flat-owned-record-api.v1";
pub const PROJECT_PROFILE_OWNED_UTF8_API_V1: &str = "owned-utf8-api.v1";
pub const PROJECT_PROFILE_NESTED_OWNED_RECORD_API_V1: &str = "nested-owned-record-api.v1";

/// Frozen Project-v4 semantic stdout authority.
pub const PROJECT_COMMAND_STDOUT_CAPABILITY: &str = "process.stdout.write";
/// Fixed-adapter authorities admitted only by Project v5.
pub const PROJECT_COMMAND_ARGS_READ_CAPABILITY: &str = "process.args.read";
pub const PROJECT_COMMAND_STDERR_WRITE_CAPABILITY: &str = "process.stderr.write";
pub const PROJECT_COMMAND_STDIN_READ_CAPABILITY: &str = "process.stdin.read";
pub const PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2: [&str; 4] = [
    PROJECT_COMMAND_ARGS_READ_CAPABILITY,
    PROJECT_COMMAND_STDERR_WRITE_CAPABILITY,
    PROJECT_COMMAND_STDIN_READ_CAPABILITY,
    PROJECT_COMMAND_STDOUT_CAPABILITY,
];
/// Fixed authority inventory admitted only by Project v12. Network authority
/// stays inert until a native socket provider or an invocation-owned fixture
/// provider is explicitly supplied by the host.
pub const PROJECT_NETWORK_COMMAND_CAPABILITIES_V1: [&str; 7] = [
    "network.connect",
    "network.read",
    "network.write",
    PROJECT_COMMAND_ARGS_READ_CAPABILITY,
    PROJECT_COMMAND_STDERR_WRITE_CAPABILITY,
    PROJECT_COMMAND_STDIN_READ_CAPABILITY,
    PROJECT_COMMAND_STDOUT_CAPABILITY,
];

/// Fixed Project-v13 authority inventory. HTTPS is one complete provider
/// operation; raw socket, TLS-key, listen, and accept authority are absent.
pub const PROJECT_HTTPS_COMMAND_CAPABILITIES_V1: [&str; 5] = [
    "network.http",
    PROJECT_COMMAND_ARGS_READ_CAPABILITY,
    PROJECT_COMMAND_STDERR_WRITE_CAPABILITY,
    PROJECT_COMMAND_STDIN_READ_CAPABILITY,
    PROJECT_COMMAND_STDOUT_CAPABILITY,
];

/// Exact fixed input adapter selected by Project v5.
pub const PROJECT_COMMAND_INPUT_V1: &str = "stdin-bytes+one-utf8-arg.v1";
/// Exact immutable invocation snapshot selected only by Project v6.
pub const PROJECT_LANGUAGE_COMMAND_INPUT_V1: &str = "argv-utf8+stdin-bytes.v1";

/// One exact Project profile selected by the manifest schema. This enum is the
/// authority passed to project linking and backend preparation; callers must
/// not infer profile semantics from a schema comparison or boolean flag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectProfile {
    ScalarV1,
    UsefulTextConsumerV1,
    UsefulDataV1,
    UsefulDataCommandV1,
    UsefulDataCommandV2,
    LanguageCommandIoV1,
    LineCommandIoV1,
    NetworkCommandIoV1,
    HttpsCommandIoV1,
    OwnedDataApiV1,
    FlatOwnedRecordApiV1,
    OwnedUtf8ApiV1,
    NestedOwnedRecordApiV1,
}

impl ProjectProfile {
    pub(crate) const fn is_owned_api(self) -> bool {
        matches!(
            self,
            Self::OwnedDataApiV1
                | Self::FlatOwnedRecordApiV1
                | Self::OwnedUtf8ApiV1
                | Self::NestedOwnedRecordApiV1
        )
    }

    pub const fn name(self) -> Option<&'static str> {
        match self {
            Self::ScalarV1 => None,
            Self::UsefulTextConsumerV1 => Some(PROJECT_PROFILE_USEFUL_TEXT_CONSUMER_V1),
            Self::UsefulDataV1 => Some(PROJECT_PROFILE_USEFUL_DATA_V1),
            Self::UsefulDataCommandV1 => Some(PROJECT_PROFILE_USEFUL_DATA_COMMAND_V1),
            Self::UsefulDataCommandV2 => Some(PROJECT_PROFILE_USEFUL_DATA_COMMAND_V2),
            Self::LanguageCommandIoV1 => Some(PROJECT_PROFILE_LANGUAGE_COMMAND_IO_V1),
            Self::LineCommandIoV1 => Some(PROJECT_PROFILE_LINE_COMMAND_IO_V1),
            Self::NetworkCommandIoV1 => Some(PROJECT_PROFILE_NETWORK_COMMAND_IO_V1),
            Self::HttpsCommandIoV1 => Some(PROJECT_PROFILE_HTTPS_COMMAND_IO_V1),
            Self::OwnedDataApiV1 => Some(PROJECT_PROFILE_OWNED_DATA_API_V1),
            Self::FlatOwnedRecordApiV1 => Some(PROJECT_PROFILE_FLAT_OWNED_RECORD_API_V1),
            Self::OwnedUtf8ApiV1 => Some(PROJECT_PROFILE_OWNED_UTF8_API_V1),
            Self::NestedOwnedRecordApiV1 => Some(PROJECT_PROFILE_NESTED_OWNED_RECORD_API_V1),
        }
    }
}
