# codegen/native_emit/output_profile.rs

- NativeOutputProfile · enum · L12-L33 — pub(crate) enum NativeOutputProfile
- StringRuntimeSelection · struct · L39-L43 — pub(super) struct StringRuntimeSelection
- FROZEN · constant · L46-L50 — pub(super) const FROZEN: Self = Self
- string_runtime · function · L54-L78 — pub(super) const fn string_runtime(self) -> StringRuntimeSelection
- tracks_present_strings · function · L80-L85 — pub(super) const fn tracks_present_strings(self) -> bool
- tracks_strings · function · L87-L90 — pub(super) fn tracks_strings(self, function: &ResolvedFunction) -> bool
- supports_stdout_transcript · function · L92-L104 — pub(super) const fn supports_stdout_transcript(self) -> bool
- is_command · function · L108-L121 — pub(super) const fn is_command(self) -> bool
- is_language_command · function · L125-L135 — pub(super) const fn is_language_command(self) -> bool
