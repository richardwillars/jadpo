# First-party authentication completion evidence — 2026-09-29

This is local runtime/compiler evidence for the scoped browser/API milestone.
It does not close the full authentication programme or independent review.

- [Rust workspace output](rust.log): 198 tests passed.
- [SQLite authentication output](sqlite.log): 24 tests passed.
- [PostgreSQL-mode authentication output](postgres.log): 24 tests passed.
- [Toolchain, source fingerprints and result metadata](results.json).

PostgreSQL mode exercises signed/opaque cookie and bearer variants against a
real disposable Postgres 16.3 cluster, including persistent sessions,
authoritative resolution, ownership, revocation, rotation and a separate runtime
process. The two startup-failure subprocess cases use SQLite in both runs.
This is one 24-case suite executed in two modes, not 48 distinct behaviours.

Reproduce from the repository root with PostgreSQL tools and Bun on `PATH`:

```sh
cargo test --manifest-path jadpo/Cargo.toml --workspace
cargo build --manifest-path jadpo/Cargo.toml -p jadpo-cli
./jadpo/target/debug/jadpo build examples/first-party-authentication
bun --no-install test tests/runtime/first-party-authentication.test.ts
bash tests/runtime/first-party-authentication-postgres.sh
```

The PostgreSQL runner creates, stops and removes its own localhost cluster.
The SQLite run ignores any ambient application `DATABASE_URL`. Both use
synthetic credentials. The retained output comes from the uncommitted working
tree identified by the metadata, not a clean release checkout.

Service credentials/JWT, richer principal mappings, full AUTH-P7/P8,
extended fuzzing and production/independent qualification remain open.
