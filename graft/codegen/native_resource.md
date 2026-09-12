# codegen/native_resource.rs

- RESOURCE_SYMBOL_DOMAIN · constant · L24-L24 — const RESOURCE_SYMBOL_DOMAIN: &[u8] = b"semaprax.native-resource-type.v1\0";
- FINALIZER_SYMBOL_DOMAIN · constant · L25-L25 — const FINALIZER_SYMBOL_DOMAIN: &[u8] = b"semaprax.native-finalizer-import.v1\0";
- SYMBOL_DIGEST_BYTES · constant · L26-L26 — const SYMBOL_DIGEST_BYTES: usize = 24;
- NativeResourceAbi · struct · L30-L34 — pub(super) struct NativeResourceAbi
- c_type · function · L40-L119 — pub(super) fn c_type<'a>(
- NativeResourceDescriptor · struct · L124-L127 — pub(super) struct NativeResourceDescriptor
- NativeLifecycleDescriptor · struct · L131-L136 — pub(super) struct NativeLifecycleDescriptor
- NativeFinalizerKind · enum · L142-L145 — pub(super) enum NativeFinalizerKind
- NativeImportedFinalizer · struct · L148-L153 — pub(super) struct NativeImportedFinalizer
- build_resource_abi · function · L166-L270 — pub(super) fn build_resource_abi(
- import_index · function · L272-L295 — fn import_index(
- validate_finalizer_import · function · L297-L353 — fn validate_finalizer_import(
- emit_declarations · function · L355-L404 — fn emit_declarations(
- stable_identifier · function · L406-L419 — fn stable_identifier(prefix: &str, domain: &[u8], identity: &DeclarationId) -> String
- register_identifier · function · L421-L432 — fn register_identifier(
- require_identity · function · L434-L442 — fn require_identity(kind: &str, identity: &DeclarationId) -> Result<(), Diagnostic>
- resource_error · function · L444-L446 — fn resource_error(message: impl Into<String>) -> Diagnostic
- tests · module · L450-L450 — mod tests;
