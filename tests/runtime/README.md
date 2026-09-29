# Generated runtime acceptance tests

The repository-wide entry point is `python3 tools/verify.py`; it builds all
required examples and runs these suites in separate processes, including
disposable PostgreSQL clusters. See the [validation guide](../validation/README.md).
The commands below are useful when working on an individual suite.

Build the Jadpo seed, then run its Bun HTTP acceptance suite:

```text
cd jadpo
cargo run -p jadpo-cli -- build ../examples/jadpo-seed
cargo run -p jadpo-cli -- build ../examples/outcome-sequencing
cargo run -p jadpo-cli -- build ../examples/persistence-seed
cargo run -p jadpo-cli -- build ../tests/compile/pass/120_entity_dossier_queries.jadpo
cargo run -p jadpo-cli -- build ../examples/authentication-selector
cargo run -p jadpo-cli -- build ../examples/first-party-authentication
cargo run -p jadpo-cli -- build ../examples/temporal
cargo run -p jadpo-cli -- build ../examples/policy-runtime
cargo run -p jadpo-cli -- test ../examples/test-fixtures
cd ..
bun --no-install test tests/runtime/jadpo-seed.test.ts
bun --no-install test tests/runtime/startup-failure.test.ts
bun --no-install test tests/runtime/outcome-sequencing.test.ts
bun --no-install test tests/runtime/persistence-seed.test.ts
bun --no-install test tests/runtime/entity-dossier.test.ts
bun --no-install test tests/runtime/authentication-selector.test.ts
bun --no-install test tests/runtime/first-party-authentication.test.ts
bash tests/runtime/first-party-authentication-postgres.sh
bun --no-install test tests/runtime/temporal.test.ts
bun --no-install test tests/runtime/policy-runtime.test.ts
DATABASE_URL=postgres://postgres@127.0.0.1:5432/postgres \
  bun --no-install test tests/runtime/persistence-postgres.test.ts
```

`--no-install` is mandatory evidence: generated targets and their acceptance
tests must run without package resolution, a dependency manifest, or a
`node_modules` directory.

The persistence-free outcome suite proves generated sequential execution for a
successful call, compatible local recovery, explicit failure mapping, and exact
propagation through real HTTP boundaries.

The entity-dossier suite proves that a handled nested entity action rolls its
write and authority change record back to a compiler-owned SQLite savepoint,
then permits the outer transaction to commit with contiguous per-entity change
revisions. The physical derived-store delivery adapter is deliberately outside
that claim.

The authentication-selector suite proves that credential inventory happens
before validation, zero/one/multiple and malformed presentations have stable
classifications, an invalid credential cannot hide beside a valid one, bounded
ordinary requests avoid authority lookup, and fresh checks resolve one active
principal without exposing credentials or provider objects.

The temporal suite proves strict instant normalisation, explicit London and
New York daylight-saving overlap/gap handling, 23/24/25-hour local-day bounds,
calendar-versus-elapsed arithmetic, explicit-reference friendly formatting,
closed policy options, and generated timezone/locale provenance.

The policy suite proves authoritative direct and membership role resolution,
same-scope and cross-scope concealment, field narrowing, policy predicates
before ordering/pagination and mutation, create preconditions, user and service
application roles, stable regular-user and administrator projections over one
entity, public invoke policy, and next-operation membership revocation against
an isolated SQLite database.

The authored fixture example runs through `jadpo test` itself. It proves a
fixed clock is stable within each callable operation, explicit clock advance is
visible only to the next operation, every test starts from a fresh fixture,
typed configuration is installed and restored, and `secret(...)` is accepted
only at the fixture boundary without becoming an authored runtime function.

The suite uses a real ephemeral TCP listener and covers valid input, semantic
boundary rejection, unknown-field rejection, automatic domain-failure mapping,
internal-context non-disclosure, and secret-safe structured startup failure when
the configured database cannot be reached.

The persistence suites prove the same behavior against a temporary SQLite
database and a caller-supplied fresh PostgreSQL database: invalid boundary
values write no rows, while valid `create Customer` responses match the rows
actually returned from storage. They exercise `query optional` for zero, one,
and duplicate rows; `query required` for typed not-found and successful lookup;
transactional required update/delete for zero, one, and multiple rows; declared
create/update constraint-to-conflict mapping; atomic fixed-shape multi-field
updates; and deliberate SQLite/PostgreSQL failures proving raw driver
exceptions become compiler-owned `PersistenceFault` values at the adapter
boundary. The Account pressure case separately maps a single-field unique
collision and a compound-unique collision to different domain failures while
proving the surrounding multi-field update rolls back. The SQLite suite also
inspects and executes the fresh
schema to prove generated identity, uniqueness, and lookup-index metadata.
It also proves the generated `User`–`Todo` foreign key rejects an orphan, is
indexed, and applies its declared cascade lifecycle. The PostgreSQL suite
contains the same assertions when a live `DATABASE_URL` is supplied; the P10
exit run passed all 22 PostgreSQL cases against Postgres 16.
The relationship route also proves `query many` returns zero-or-more validated
todos in explicit identity order rather than relying on database row order.

The first-party authentication suite exercises real signed/opaque cookie and
bearer adapters, protected HTTP routes, ownership-scoped persistence, expiry,
refresh, revocation, rotation, CSRF (including mutative GET), configuration
failures, request isolation, parser mutations, and audit/OpenAPI agreement.
Its 24 cases pass in both SQLite and PostgreSQL modes, including generated
immediate-cookie and signed-bearer variants, separate-process restart/revocation,
identity replacement, initialization races and startup-schema failure. The two
startup-failure subprocess cases use SQLite in both runs. The PostgreSQL runner
requires PostgreSQL tools on `PATH`, creates a disposable localhost cluster and
removes it on exit. SQLite mode ignores ambient `DATABASE_URL`; no existing
application database is used. See the [example](../../examples/first-party-authentication/README.md).
