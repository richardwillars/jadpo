# Generated runtime acceptance tests

Build the Jadpo seed, then run its Bun HTTP acceptance suite:

```text
cd jadpo
cargo run -p jadpo-cli -- build ../examples/jadpo-seed
cd ..
bun --no-install test tests/runtime/jadpo-seed.test.ts
bun --no-install test tests/runtime/persistence-seed.test.ts
DATABASE_URL=postgres://postgres@127.0.0.1:5432/postgres \
  bun --no-install test tests/runtime/persistence-postgres.test.ts
```

`--no-install` is mandatory evidence: generated targets and their acceptance
tests must run without package resolution, a dependency manifest, or a
`node_modules` directory.

The suite uses a real ephemeral TCP listener and covers valid input, semantic
boundary rejection, unknown-field rejection, automatic domain-failure mapping,
and internal-context non-disclosure.

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
