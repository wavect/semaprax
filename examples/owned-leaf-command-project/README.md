# Owned leaf command project

This Project v30 example pairs an ordinary explicit stdin/stdout command route
with a flat owned-leaf collection. `Entry` has a `string` and an `i64`; the
library sorts the vector, deep-clones one entry for inspection, replaces an
entry, and consumes the remaining vector with `for own`.

The project profile admits the owned-leaf collection runtime. The command
reads the stdin stream and writes its byte count; the collection demonstration
runs through the ordinary entry and test modules. No JSON codec or public
owned-data API is used.
