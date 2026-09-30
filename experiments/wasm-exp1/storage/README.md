# Common synchronous storage adapter (experiment only)

This adapter consumes the checked executable projection. It derives persistent
entities, field types/constraints/nullability, identity, uniqueness, immutable
fields, allowed operation write fields, direct role bindings and operation policy
obligations from that projection. It has no application/entity/owner-field name
branches. A renamed schema/principal/binding-field test exercises that property.
It neither interprets application statements nor selects application recovery arms.

## Integration API

```ts
const storage = createStorage(program, bunSqlite(disposableDatabase));
// Inside a SQLite-backed Durable Object, the same API uses:
// const storage = createStorage(program, cloudflareSqlite(this.ctx.storage));
storage.setup(); // Creates only the checked persistent schema.

// This value comes from the trusted harness, never the incoming request body.
const principal = { entity: "User", values: { id: trustedIdentity } };

// Root driver invokeSync must run the whole start/resume loop synchronously here.
const terminal = storage.runAtomic(() => invokeSync(/* ... */, pending => {
  if (pending.capability === "storage.read") return storage.read(pending.args, principal);
  if (pending.capability === "storage.update") return storage.update(pending.args, principal);
  throw new Error("unsupported capability");
}));
```

The names in the principal example belong to the test projection, not adapter
constants: the adapter reads `principalEntity` from each checked binding and its
identity field from the checked entity declaration.

`SqlAdapter` has three synchronous methods: `rows(sql, boundParameters)`,
`transactionSync(callback)`, and `isUniqueConflict(error)`. Both concrete adapters
consume complete query results before returning. The Bun adapter uses an immediate
SQLite transaction; the Cloudflare adapter calls `storage.transactionSync` and
`storage.sql.exec(...).toArray()`.

Read arguments retain the probe's existing shape:

```ts
{
  entity, operation, semanticOperationId,
  freshness: "authoritative",
  policy: storage.descriptor(operation),
  predicate: { field, operator: "equal", value }
}
```

The supplied policy descriptor must exactly match the trusted checked operation,
all checked bindings and entity rules; it cannot weaken them. The semantic ID and
operation name must agree. The predicate must match the checked storage operation
and its identity field. Every SQL value, including the trusted principal restriction,
is a bound parameter. Identifiers are separately validated and quoted.

- `read(args, principal)` returns `{kind:"success",value:row|null}`.
- `update({...args, changes}, principal)` returns
  `{kind:"success",value:{status:"found",row}}`, or a value with `status` equal to
  `missing`, `conflict`, or `empty`. These are storage outcomes, not domain failures.
  The generated core must map them to its authored arms. The outer resume envelope
  stays within the frozen `success`/`domain`/`internal` protocol.
- Updates require an active `runAtomic` boundary. `runAtomic(work)` commits only a
  terminal `kind:"success"`. For terminal `domain`, `internal`, or `invalid`, it
  throws a private marker inside the database callback to force rollback, catches
  that marker outside the transaction, and returns the original terminal envelope.
  Returning an application domain failure directly from `transactionSync` would
  commit; callers must use this wrapper.
- The whole Wasm loop must finish before the callback returns. Promise/thenable,
  pending/unknown terminal result and nested adapter transaction are rejected.
  The outer Durable Object RPC await supplies the asynchronous host boundary; an
  arbitrary remote await inside this callback is unsupported.
- `schema` exposes entity/table/identity/field descriptors. `resetFixture(rows)` and
  `snapshot()` are trusted test setup/evidence helpers, not Wasm capabilities or
  public production endpoints.

Unsupported/malformed host arguments and malformed stored rows throw
`UnsupportedStorage` rather than returning invalid client input or a domain failure.
The outer driver must contain that error as an internal fault. Its raw detail must
not be put into public output. This is infrastructure, not an authentication adapter.

## Deliberately bounded capabilities

This slice supports one authority store, direct entity role bindings, required
identity-equality reads, and checked patch/set updates over Text/Uuid fields.
It rejects unsupported field types/constraints, indirect or field-level policy,
weaker/unavailable authoritative freshness, cross-store transactions, undeclared
writes, identity/immutable changes and handled nested mutation outcome scopes.
A checked graph whose query plan is not authoritative is rejected before setup.
This is a closed experimental subset, not a complete general persistence backend.
No joins, migration protocol, cross-object atomicity, replicated database or async
service call is supplied. Fixture corruption tests exercise malformed stored values.

## Evidence and reproduction

```sh
bun --no-install --env-file=/dev/null test experiments/wasm-exp1/storage/adapter.test.ts
WRANGLER_SEND_METRICS=false WRANGLER_LOG_PATH=build/wasm-exp1/storage/types.log experiments/wasm-exp1/tooling/node_modules/.bin/wrangler types build/wasm-exp1/storage/worker-configuration.d.ts --config experiments/wasm-exp1/storage/wrangler.jsonc --env-file /dev/null
WRANGLER_SEND_METRICS=false WRANGLER_LOG_PATH=build/wasm-exp1/storage/dev.log experiments/wasm-exp1/tooling/node_modules/.bin/wrangler dev --local --config experiments/wasm-exp1/storage/wrangler.jsonc --port 19875 --inspector-port 19876 --persist-to build/wasm-exp1/storage/state --show-interactive-dev-session=false --env-file /dev/null
# In another shell, while the local server runs:
curl --fail http://127.0.0.1:19875
```

`local-worker.ts` and `wrangler.jsonc` are a **local-only infrastructure smoke test**;
no deployment was made. Its RPC invokes the adapter against actual workerd SQLite
Durable Object storage and performs a second RPC to observe committed state. It
uses generated Wrangler types under ignored build output. No full Wasm slice is
being executed by this smoke, so these checks cannot count as full-slice/cloud
acceptance passes. The root experiment owns generated core integration/deployment.

The focused Bun run passes **14 tests / 49 assertions**. Local workerd returns
HTTP 200 with **9 checks**, covering scoped reads, outsider isolation, domain rollback,
uniqueness classification, commit/null handling and observation through a later RPC.
Raw evidence is under `build/wasm-exp1/storage/`. The first local attempt exposed
workerd's extended uniqueness message suffix; the adapter was corrected to match
that exact unique/primary-key signature, and tests exclude other SQLite failures.
No driver-error text escapes as a storage conflict payload.

Official references consulted on 2026-09-30:

- [SQLite-backed Durable Object storage](https://developers.cloudflare.com/durable-objects/api/sqlite-storage-api/#transactionsync): synchronous callbacks and exception rollback.
- [SQL API](https://developers.cloudflare.com/durable-objects/api/sqlite-storage-api/#exec): bound parameters and complete synchronous cursor consumption.
- [Workers best practices](https://developers.cloudflare.com/workers/best-practices/workers-best-practices/).

Active implementation/verification time was approximately ten minutes, plus roughly
30 seconds of tool waits, charged to common full-slice preparation. This is an
accounting estimate; no performance comparison is inferred from it.
