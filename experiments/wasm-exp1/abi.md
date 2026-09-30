# Probe ABI v1 (freeze before either route)

Single-threaded core Wasm, no WASI, no threads, no shared memory, no JSPI.
The compiler consumes schemaVersion 1 checked-model projection. Both routes
must generate application control flow from it. Host glue may encode/decode
values and dispatch declared storage capabilities; it must not interpret
application statements or select recovery arms for the application.

Each request owns its own instance and linear memory. Compiled modules may be
shared. This deliberately simple lifetime model is measured as part of startup
and memory costs. It is not a claim about an optimized production allocator.
All byte buffers are UTF-8, bounded to 65,536 bytes individually. Pointer and
length checks must reject overflow or access outside current memory. No host
retains a borrowed memory view across suspension. Copy returned bytes before
awaiting. Memory maximum: 128 pages (8 MiB), including Rust stack/heap where used.

Exports:

- `memory`: bounded linear memory.
- `alloc(length: i32) -> i32`: request-owned buffer; zero on unavailable capacity.
- `start(operation: i32, input_ptr: i32, input_len: i32) -> i32`.
- `resume(request_id: i32, operation_id: i32, result_ptr: i32, result_len: i32) -> i32`.
- `result_ptr() -> i32`, `result_len() -> i32`: borrowed output envelope.

Status values: 0 completed, 1 declared domain failure, 2 pending host operation,
3 unexpected internal failure, 4 invalid boundary input. A completed or failed
frame cannot resume. Duplicate/mismatched resume must fail closed. The one
active request frame owns monotonically assigned pending operation IDs. Host
glue checks those IDs before invoking resume; Wasm independently verifies them.
A trap discards the instance; it cannot turn into a declared failure/recovery.

JSON envelopes preserve omitted property, explicit `null`, and present value.
The host performs bounded parsing/encoding; generated semantic descriptors
validate nominal types, field presence and closed shapes. Each route records
which validation executes inside Wasm versus compiler-owned glue. Neither may
accept invalid input/result because the other trusted component supplied it.
Successful output: `{kind:"success",value:...}`. Declared failure:
`{kind:"domain",failure:<declared name>}`. Internal failure:
`{kind:"internal",operation:<semantic ID>}` with generic public text only.
Host pending: `{kind:"pending",requestId,operationId,capability,args}`. Resume
input is a closed success/domain/internal envelope tied to the pending IDs;
malformed host data produces an internal fault, not a client validation error.

A generator may omit unused exports for a demonstrably pure entry point, but
both complete probes must exercise the pending/resume boundary and all exports.
Static string data, generated operation dispatch and request frame layouts are
route implementation details. Any ABI change before probe execution requires a
new common freeze. Any change after execution must be recorded as a protocol
deviation and applied/retested equally, not silently used to favor a route.
