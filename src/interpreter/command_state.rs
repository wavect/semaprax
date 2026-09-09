//! Invocation-owned injected host state; absent authority remains absent.
use super::{environment, filesystem, network};
use std::sync::Arc;
pub(super) struct CommandInputState<'a> {
    pub(super) network: Option<network::NetworkState<'a>>,
    pub(super) filesystem: Option<filesystem::FileState<'a>>,
    pub(super) environment: Option<environment::EnvironmentState>,
    pub(super) arguments: Vec<Arc<[u8]>>,
    pub(super) stdin: Arc<[u8]>,
    pub(super) stdin_consumed: bool,
}
