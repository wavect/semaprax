# Allocation-free frontier observations outside static TLS

The exact original 128 KiB constructor test still faulted in Linux run
39826150007 at dbf6fed71b after removing optional PLAN rows from static TLS.
The actual ELF TLS segment was 0x39570 bytes (234864). The remaining test-only
4096-event frontier bank contributes 196608 bytes on this 64-bit host.

The complete fixed event bank now lives in static storage with a mutex lease.
Explicit frontier observations serialize between test threads. A thread retains
its exclusive lease until thread exit, preserving repeated finish snapshots.
Inactive compiler callbacks acquire no lease, never lock, and never allocate.
The original Event and Trace field declarations and 4096-event capacity are
unchanged. No heap-backed bank, capacity reduction, or compiler allowance is
introduced. These are private test observations, not production concurrency or
performance qualification.

The unchanged original fresh-thread test still measures zero actual heap
allocations on first reset and checks all 4096 events, the one-over refusal,
and swap filtering. Two added checks exercise inactive callbacks at 128 KiB
and refusal to attribute another thread's events, including repeated snapshots.
All three pass in the actual observer/ledger harness. The original Linux test
and full CI remain separate gates; the prior diagnostic capture is a failure.
