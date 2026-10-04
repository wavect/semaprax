# Excluded setup lineage: wrong byte identity sidecar

This bounded 30+30 capture completed its child processes, but its sidecar
recorded a semantic source digest where the capsule requires the exact source
byte SHA-256. The offline verifier rejected it, so it is not an assessed
process-state result. The corrected v3 capsule binds the returned source bytes
and is the only assessed Z3 process-state corpus.
