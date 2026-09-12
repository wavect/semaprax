# process_provider/registered/physical_tests.rs

- fixture · function · L6-L49 — fn fixture() -> &'static Path
- ROOT · constant · L7-L7 — static ROOT: OnceLock<PathBuf> = OnceLock::new();
- provider · function · L50-L61 — fn provider(policy: fn(&[Vec<u8>]) -> bool) -> RegisteredProcessProvider
- request · function · L62-L69 — fn request(args: &[&[u8]], input: &[u8], timeout: u64, out: usize, err: usize) -> ProcessRequest
- physical_registered_process_explicit_authority_and_nonzero_exit · function · L71-L85 — fn physical_registered_process_explicit_authority_and_nonzero_exit()
- physical_registered_process_simultaneous_pipes_raw_arguments_and_signal · function · L87-L105 — fn physical_registered_process_simultaneous_pipes_raw_arguments_and_signal()
- physical_registered_process_timeout_overflow_and_recovery · function · L107-L125 — fn physical_registered_process_timeout_overflow_and_recovery()
- physical_registered_process_invalid_executable_is_launch_failure · function · L127-L143 — fn physical_registered_process_invalid_executable_is_launch_failure()
- physical_registered_process_uses_held_executable_after_path_replacement · function · L146-L168 — fn physical_registered_process_uses_held_executable_after_path_replacement()
