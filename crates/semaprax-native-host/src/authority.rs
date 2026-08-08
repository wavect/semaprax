//! Same-thread authenticated capability authority bound to one physical lease.

#![forbid(unsafe_code)]

use std::thread::{self, ThreadId};

use crate::native_capability_token::{
    authenticate_expected, mint, NativeCapabilityBinding, NativeCapabilityKind,
    NativeCapabilitySecret, NativeCapabilityTokenError, TOKEN_BYTES,
};
use semaprax_native_loader::NativeModuleLease;

const SECRET_BYTES: usize = 32;
const EPOCH_BYTES: usize = 8;
const THREAD_NONCE_BYTES: usize = 32;
const SEED_BYTES: usize = SECRET_BYTES + EPOCH_BYTES + THREAD_NONCE_BYTES;
const THREAD_BINDING_HEX_BYTES: usize = THREAD_NONCE_BYTES * 2;

pub(crate) struct Authority {
    secret: NativeCapabilitySecret,
    module_lease: NativeModuleLease,
    physical_module: [u8; 32],
    adapter_identity: Vec<u8>,
    binding_epoch: u64,
    thread_policy_identity: &'static [u8],
    thread_binding_identity: [u8; THREAD_BINDING_HEX_BYTES],
    bound_thread: ThreadId,
}

pub(crate) struct Credential {
    bytes: [u8; TOKEN_BYTES],
    module_lease: NativeModuleLease,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AuthorityError {
    EntropyUnavailable,
    InvalidEntropy,
    InvalidBinding,
    WrongThread,
    WrongModuleInstance,
    Token(NativeCapabilityTokenError),
}

impl Authority {
    pub(crate) fn from_os(
        module_lease: NativeModuleLease,
        physical_module: [u8; 32],
        adapter_identity: &[u8],
    ) -> Result<Self, AuthorityError> {
        if physical_module == [0; 32]
            || adapter_identity.is_empty()
            || adapter_identity.contains(&0)
        {
            return Err(AuthorityError::InvalidBinding);
        }
        let mut seed = [0_u8; SEED_BYTES];
        if getrandom::fill(&mut seed).is_err() {
            seed.fill(0);
            return Err(AuthorityError::EntropyUnavailable);
        }
        let mut secret_bytes = [0_u8; SECRET_BYTES];
        secret_bytes.copy_from_slice(&seed[..SECRET_BYTES]);
        let binding_epoch = u64::from_le_bytes(
            seed[SECRET_BYTES..SECRET_BYTES + EPOCH_BYTES]
                .try_into()
                .expect("authority epoch has a fixed width"),
        );
        let mut thread_nonce = [0_u8; THREAD_NONCE_BYTES];
        thread_nonce.copy_from_slice(&seed[SECRET_BYTES + EPOCH_BYTES..]);
        seed.fill(0);
        if secret_bytes.iter().all(|byte| *byte == 0)
            || binding_epoch == 0
            || thread_nonce.iter().all(|byte| *byte == 0)
        {
            secret_bytes.fill(0);
            thread_nonce.fill(0);
            return Err(AuthorityError::InvalidEntropy);
        }
        let secret = NativeCapabilitySecret::from_trusted_runtime_entropy(secret_bytes)
            .map_err(|_| AuthorityError::InvalidEntropy)?;
        secret_bytes.fill(0);
        let thread_binding_identity = encode_lower_hex(&thread_nonce);
        thread_nonce.fill(0);
        let authority = Self {
            secret,
            module_lease,
            physical_module,
            adapter_identity: adapter_identity.to_vec(),
            binding_epoch,
            thread_policy_identity: b"semaprax.native-host.same-thread.v1",
            thread_binding_identity,
            bound_thread: thread::current().id(),
        };
        // Prove the static binding is valid before returning live authority.
        authority.binding(
            NativeCapabilityKind::Owner,
            None,
            b"validation",
            b"validation",
        )?;
        Ok(authority)
    }

    pub(crate) fn mint_owner(
        &self,
        resource: &[u8],
        lifecycle: &[u8],
        slot: u64,
        generation: u64,
    ) -> Result<Credential, AuthorityError> {
        self.mint(
            NativeCapabilityKind::Owner,
            None,
            resource,
            lifecycle,
            slot,
            generation,
        )
    }

    pub(crate) fn authenticate_owner(
        &self,
        credential: &Credential,
        resource: &[u8],
        lifecycle: &[u8],
        slot: u64,
        generation: u64,
    ) -> Result<(), AuthorityError> {
        self.authenticate(
            NativeCapabilityKind::Owner,
            None,
            credential,
            resource,
            lifecycle,
            slot,
            generation,
        )
    }

    pub(crate) fn mint_result(
        &self,
        function_template: &[u8; 32],
        resource: &[u8],
        lifecycle: &[u8],
        slot: u64,
        generation: u64,
    ) -> Result<Credential, AuthorityError> {
        self.mint(
            NativeCapabilityKind::FunctionOwnedResult,
            Some(function_template),
            resource,
            lifecycle,
            slot,
            generation,
        )
    }

    pub(crate) fn authenticate_result(
        &self,
        function_template: &[u8; 32],
        credential: &Credential,
        resource: &[u8],
        lifecycle: &[u8],
        slot: u64,
        generation: u64,
    ) -> Result<(), AuthorityError> {
        self.authenticate(
            NativeCapabilityKind::FunctionOwnedResult,
            Some(function_template),
            credential,
            resource,
            lifecycle,
            slot,
            generation,
        )
    }

    pub(crate) fn is_same_instance(&self, credential: &Credential) -> bool {
        self.module_lease.is_same_instance(&credential.module_lease)
    }

    fn mint(
        &self,
        kind: NativeCapabilityKind,
        function_template: Option<&[u8; 32]>,
        resource: &[u8],
        lifecycle: &[u8],
        slot: u64,
        generation: u64,
    ) -> Result<Credential, AuthorityError> {
        self.require_current_thread()?;
        let module_lease = self.module_lease.retain();
        let binding = self.binding(kind, function_template, resource, lifecycle)?;
        let bytes =
            mint(&self.secret, &binding, slot, generation).map_err(AuthorityError::Token)?;
        Ok(Credential {
            bytes,
            module_lease,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn authenticate(
        &self,
        kind: NativeCapabilityKind,
        function_template: Option<&[u8; 32]>,
        credential: &Credential,
        resource: &[u8],
        lifecycle: &[u8],
        slot: u64,
        generation: u64,
    ) -> Result<(), AuthorityError> {
        self.require_current_thread()?;
        if !self.is_same_instance(credential) {
            return Err(AuthorityError::WrongModuleInstance);
        }
        let binding = self.binding(kind, function_template, resource, lifecycle)?;
        authenticate_expected(&self.secret, &binding, &credential.bytes, slot, generation)
            .map(|_| ())
            .map_err(AuthorityError::Token)
    }

    fn binding<'a>(
        &'a self,
        kind: NativeCapabilityKind,
        function_template: Option<&'a [u8; 32]>,
        resource: &'a [u8],
        lifecycle: &'a [u8],
    ) -> Result<NativeCapabilityBinding<'a>, AuthorityError> {
        NativeCapabilityBinding::from_trusted_runtime_binding(
            &self.physical_module,
            &self.adapter_identity,
            self.binding_epoch,
            kind,
            function_template,
            resource,
            lifecycle,
            self.thread_policy_identity,
            &self.thread_binding_identity,
        )
        .map_err(|_| AuthorityError::InvalidBinding)
    }

    fn require_current_thread(&self) -> Result<(), AuthorityError> {
        if thread::current().id() == self.bound_thread {
            Ok(())
        } else {
            Err(AuthorityError::WrongThread)
        }
    }
}

impl Credential {
    pub(crate) fn instance_id(&self) -> semaprax_native_loader::ModuleInstanceId {
        self.module_lease.instance_id()
    }
}

fn encode_lower_hex(input: &[u8; THREAD_NONCE_BYTES]) -> [u8; THREAD_BINDING_HEX_BYTES] {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = [0_u8; THREAD_BINDING_HEX_BYTES];
    for (index, byte) in input.iter().copied().enumerate() {
        output[index * 2] = HEX[usize::from(byte >> 4)];
        output[index * 2 + 1] = HEX[usize::from(byte & 0x0f)];
    }
    output
}
