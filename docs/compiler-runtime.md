# Jadpo compiler and runtime architecture

**Status:** exploratory architecture  
**Goal:** preserve implementation conclusions without prematurely committing to
a production compiler design

## 1. Architecture follows the semantic model

The implementation should not begin with a polished grammar. The intended
pipeline is conceptually:

```text
source
  -> parser
  -> syntax tree
  -> semantic model
  -> type and boundary validation
  -> policy checker
  -> effect/audit graph
  -> generated artifacts
  -> executable target
```

The valuable component is the semantic model and its checks. The parser and
target are replaceable implementation choices.

## 2. Toolchain language

Rust was the leading suggestion for the core toolchain because compiler tooling,
safety, performance, and a possible native-runtime future all fit it well. Zig
was raised as another possible systems language but not explored in detail.

This is provisional. The first experiment need not prove that a production Rust
compiler can be built; it must prove the programming model.

## 3. Execution strategy

### 3.1 First practical target

A pragmatic initial route is:

- Rust (or another suitable implementation language) for parsing and semantic
  checks;
- generated TypeScript as an implementation artifact;
- Bun or a similar runtime;
- Postgres as the database;
- an intentionally boring generated runtime.

This avoids rebuilding an HTTP server, database driver, TLS, observability,
queues, and deployment ecosystem before the hypothesis is validated.

The first prototype could be interpreted rather than compiled. Native code,
performance, and LLVM are not proof of the design.

### 3.2 Possible long-term target

A native executable with Postgres as its main infrastructure assumption would
give the language a cleaner identity and deployment story:

```text
backend build
  -> myapp
  -> migrations
  -> OpenAPI
  -> docs
  -> audit
```

This may be more elegant and commercially distinctive, but it is a later
technical problem. The initial reason to switch is correctness, constrained
evolution, and agent safety—not performance.

## 4. Build outputs

P8 and P9 establish one compiler-owned disposable output root:

```text
build/
  app.meta.json
  inventory/
  audit/
  validators/
  compatibility/
  openapi/
  target/
    app.ts                 generated Bun entry point
```

`build/` is excluded from source discovery and ignored by the canonical
scaffold. The compiler must be able to reproduce it from authored source, so no
file beneath it is an authoritative input or normal review surface. Source maps
and deployment packaging remain future additions; they must stay within the
same boundary rather than creating a second generated tree.

## 5. Static checks

The compiler should eventually include:

- parsing and name/type resolution;
- constraint and exhaustiveness checks;
- effect tracking;
- optional-value narrowing;
- query cardinality and projection typing;
- policy/authorisation proof or conformance analysis;
- secret/data-flow checks;
- declared-egress checks;
- lifecycle and migration impact analysis;
- transaction and idempotency analysis;
- resource-bound checks;
- output exposure checks;
- generation consistency and internal validation.

Compiler errors should describe the domain decision or violated invariant, not
dump the details of generated framework code.

## 6. Runtime validation

Compile-time typing is insufficient because backend data crosses untrusted
runtime boundaries. Validators are generated from the same declarations and run
on:

- HTTP requests;
- HTTP responses;
- database reads;
- database writes;
- environment/config values;
- queue and event payloads;
- external API responses;
- cached/deserialised data.

A Postgres row that does not satisfy the expected entity type is a hard runtime
contract failure. It must not become a malformed trusted object.

Domain constraints should also become database `CHECK` constraints where
possible. One rule should not be hand-copied into code, Zod, SQL, OpenAPI, docs,
and tests.

## 7. Database runtime

Postgres is the opinionated initial default. Ordinary source uses declarative,
typed queries and mutations. The runtime owns:

- parameterisation and safe SQL generation;
- connection handling;
- transaction execution;
- serialization/deserialization;
- constraint error translation;
- migration execution;
- observability and timeouts.

Complex safe query constructs should be added based on real applications.
Raw SQL, if eventually necessary, is a deliberate audited escape hatch.

## 8. External service runtime

Service operations are generated from reviewed contracts rather than hand-made
client libraries. The runtime owns:

- authentication and secret injection;
- request/response validation;
- timeouts and retry policy;
- idempotency support;
- provider-error classification;
- observability and redaction;
- permitted destination/egress enforcement.

Provider failures are translated at this boundary into typed domain failures or
operational faults.

## 9. Source mapping and semantic metadata

Generated TypeScript must never be the normal developer-facing source. Ordinary
source maps are necessary but insufficient because generated code often has no
one-to-one source line.

Every important semantic node should receive a stable identifier, for example:

```text
route:todos:get-one
query:todos:get-one:load
policy:todos:get-one:owner
output:todos:get-one
```

Generated target sections retain both source ranges and semantic node IDs:

```text
generated lines 145-163
  source: todos.<source>:5
  node: route:todos:get-one:owner-check
```

`app.meta` can map runtime operations, target locations, source locations,
entities, fields, policies, and effects. Maps may be uploaded to a monitoring
service during deployment rather than publicly shipped.

## 10. Diagnostic categories

### 10.1 Source-language compile errors

These are application errors expressed entirely in language concepts:

```text
ERROR AUTH003

GET /todos/:id returns Todo but does not prove
that the current user may read the selected record.

todos.<source>:14
```

### 10.2 Generated-target compile errors

Valid source producing invalid TypeScript is a compiler bug, not an application
error. The developer should see an internal compiler diagnostic with the source
node and a report reference. They must not be asked to edit generated code.

### 10.3 Production runtime errors

Runtime failures are caught, enriched, and mapped to source concepts:

```text
ERROR

app: TodoApp
route: GET /todos/:id
operation: Todo lookup
entity: Todo
source: todos.<source>:14
request: req_28a91

cause:
  database connection timed out

duration:
  5000ms
```

The low-level generated stack remains available to compiler/runtime maintainers
but is not the primary diagnostic.

Production handling follows the [failure model](failure-model.md):

- declared domain failures use their standard kind to select the boundary
  response and record a semantic propagation trace without a native stack by
  default;
- operational faults retain a normalised cause chain and low-level stack
  internally while returning a generic safe response;
- defects and unclassified target exceptions are containment failures, return a
  generic internal response, and raise an internal invariant diagnostic;
- only fields declared in a failure's public schema reach the client;
- internal diagnostic fields remain subject to type-aware redaction and
  retention policy.

## 11. Errors without a literal source line

Default route authentication may expand to a large generated component despite
having no explicit source line. When it fails, the runtime should report the
route and inherited security rule rather than pretend an arbitrary generated
line is meaningful:

```text
Authentication runtime failure

GET /account

Declared at:
  account.<source>:7
  route GET /account

Inherited rule:
  authentication required by default

Internal component:
  session.verify

Cause:
  session store unavailable
```

Semantic mapping can make debugging better than ordinary TypeScript because the
runtime knows what the application was trying to do.

## 12. Agent-facing diagnostics

Diagnostics should minimise the context an agent must ingest while actively
guiding it toward a valid repair. Prefer:

```text
Required Todo lookup failed in GET /todos/:id
todos.<source>:17

Expected one Todo.
Database returned zero.

Related declarations:
- Todo entity: entities.<source>:8
- route: todos.<source>:11
```

over a target stack such as:

```text
TypeError at generated/runtime/db/query.ts:874
```

The implemented direction is one catalogue-backed semantic diagnostic with a
human-first summary, reason, recommended next step, bounded alternatives,
readable lower-case dotted rule identifier, precise location, typed context,
bounded impact, decision owner, help identifier, and source revision. A proposed
edit is marked preferred only when the compiler can justify it, preview its
behavioural and public-contract effect, and validate it against that revision.

The LLM packet is intentionally richer than production telemetry. Safe runtime
events carry correlation, semantic operation, and source-revision IDs; a trusted
local tool joins those IDs with the compiler manifest to recover source
location, call graph, affected routes, declared problem sets, and repair choices.
Raw customer values, credentials, provider errors, SQL, request bodies, and
headers are neither required nor permitted in that join.

The LSP renders the same object progressively: summary in the Problems panel,
cause and recommendation on hover, preferred verified edit plus alternatives in
Quick Fix, and impact/documentation in an expandable detail view. Editor plugins
do not construct their own advice.

`CompilerDiagnostic`, `PublicFailureResponse`, `OperationalLogEvent`, and
`AgentIncidentPacket` remain distinct types. Logging and diagnostic APIs accept
only compiler-approved safe values; secret values cannot be rendered, and an
internal failure field is not automatically loggable.

## 13. Formatting

Formatting is non-semantic. A canonical formatter gives stable diffs and a
single representation.

The discussion considered formatting on every save, then identified a tool-use
risk: rewriting a file while an agent is making iterative position-based edits
can invalidate its view. The preferred direction is checkpoint formatting:

- explicit format command;
- compilation boundary;
- pre-commit or pre-merge;
- CI verifies canonical form.

The agent drafts, the tool normalises, then the next iteration starts from the
canonical result.

## 14. Operational integrations

Structured semantic logs should integrate with systems such as Sentry or
Datadog while retaining the low-level trace underneath. The runtime should
provide consistent request IDs, operation/entity context, source node, duration,
redaction, and failure classification.

Observability must not become a route for secrets or private fields to escape.
Stable failure codes and kinds may be bounded metric dimensions; arbitrary
messages, IDs, and provider payloads must not become labels.

## 15. Implementation sequence

The active sequence, evidence, and exit gates live in the
[compiler implementation roadmap](implementation-roadmap.md). In summary:

1. freeze a Jadpo seed, core grammar, and machine-readable fixtures;
2. build the parser, semantic graph, type checker, and failure checker;
3. emit semantic artifacts before executable target code;
4. generate the smallest TypeScript/Bun runtime slice;
5. require the complete todo source before persistence/auth expansion;
6. use the order/payment application and TypeScript baseline to decide whether
   to continue;
7. defer native compilation and production deployment until the language model
   proves itself.

## 16. Failure condition

If users or agents routinely inspect generated `.ts` files to understand or fix
application behaviour, the source mapping and abstraction have failed.
