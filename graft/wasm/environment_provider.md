# wasm/environment_provider.mjs

- createEnvironmentProvider · function · L9-L91 — function createEnvironmentProvider({environment = null, arguments: argv = [], stdin = new Uint8Array()} = {})
- fail · function · L11-L11 — fail = message
- text · function · L12-L30 — text = (value, name)
- debit · function · L35-L35 — debit=bytes
- compare · function · L45-L45 — compare=(a,b)
- carrier · function · L50-L50 — carrier=(root,length)
- output · function · L51-L56 — output=(pointer,size)
- put64 · function · L57-L57 — put64=(pointer,value)
- index · function · L58-L58 — index=(value,count)
- lookup · function · L59-L63 — lookup=(value,pointer,field)
- spx_environment_len_v1 · method · L65-L65 — spx_environment_len_v1(pointer)
- spx_command_arg_utf8_v1 · method · L69-L69 — spx_command_arg_utf8_v1(value,pointer)
- spx_command_stdin_read_v1 · method · L70-L77 — spx_command_stdin_read_v1(pointer)
- spx_command_owned_bytes_validate_v1 · method · L78-L78 — spx_command_owned_bytes_validate_v1(value)
- attach · method · L81-L89 — attach(target,allocator={})
- publish · function · L86-L86 — publish=bytes
