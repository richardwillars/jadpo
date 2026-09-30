# Authenticated shared-Rust policy experiment

Local follow-up to the shared-core conformance slice. The previous native/WASM
experiments remain frozen; Bun remains the default target. This is not a
production migration or authentication security qualification.

The checked fixture uses existing language features: opaque bearer credentials,
immediate revocation, live user resolution, direct owner/editor roles, narrowing
field read/update policies, and atomic paired writes/domain rejection. Rust and
WASM share the generated application plus credential verification, route
validation, policy SQL construction and failure mapping. Existing rusqlite and
Bun SQLite drivers execute prepared SQL; the Bun reference runs the unmodified
generated application/authentication/persistence with a budget wrapper.

Build without Bun (Python, Cargo, native SQLite development library and the
`wasm32-unknown-unknown` Rust target are required):

```sh
jadpo build experiments/auth-policy/fixture --target native
jadpo build experiments/auth-policy/fixture --target wasm
```

Direct shell equivalents are `sh experiments/auth-policy/build.sh native` and
`sh experiments/auth-policy/build.sh wasm`. These also work without an installed
Jadpo CLI. With no target argument the script builds both.

Builds use pinned offline dependencies. For an empty Cargo cache, first fetch the
locked dependencies for this workspace and `compiler/Cargo.toml`. The checked
projection compiler generates Bun reference source as data; building either Rust
target does not execute Bun. Outputs live under ignored `build/` directories.

The comparison harness uses Bun to issue **synthetic** credentials through the
existing generated authentication host and create disposable SQLite databases.
The native executable needs only its configured SQLite file and secrets at runtime.
Servers bind loopback; harness-only database controls require a separate test token.
Never deploy these benchmark/control adapters.

Excluded: signed/browser credentials, CSRF, JWT, service authentication,
memberships, indirect scopes, general project compilation, nested handled-write
savepoints, cloud/workerd testing and performance claims. Unsupported checked
plans must fail compilation. SQL traces separate auth reads, policy predicates,
transaction controls and generated Bun's additional reads/savepoints.
