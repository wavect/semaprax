/// Additive Project profile names. The manifest schema selects exactly one
/// profile; downstream consumers receive this closed enum rather than infer
/// authority from a schema string or boolean flag.
pub const PROJECT_PROFILE_USEFUL_TEXT_CONSUMER_V1: &str = "useful-text-consumer.v1";
pub const PROJECT_PROFILE_USEFUL_DATA_V1: &str = "useful-data.v1";
pub const PROJECT_PROFILE_USEFUL_DATA_COMMAND_V1: &str = "useful-data-command.v1";
pub const PROJECT_PROFILE_USEFUL_DATA_COMMAND_V2: &str = "useful-data-command.v2";

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

/// Exact fixed input adapter selected by Project v5.
pub const PROJECT_COMMAND_INPUT_V1: &str = "stdin-bytes+one-utf8-arg.v1";

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
}

impl ProjectProfile {
    pub const fn name(self) -> Option<&'static str> {
        match self {
            Self::ScalarV1 => None,
            Self::UsefulTextConsumerV1 => Some(PROJECT_PROFILE_USEFUL_TEXT_CONSUMER_V1),
            Self::UsefulDataV1 => Some(PROJECT_PROFILE_USEFUL_DATA_V1),
            Self::UsefulDataCommandV1 => Some(PROJECT_PROFILE_USEFUL_DATA_COMMAND_V1),
            Self::UsefulDataCommandV2 => Some(PROJECT_PROFILE_USEFUL_DATA_COMMAND_V2),
        }
    }
}
