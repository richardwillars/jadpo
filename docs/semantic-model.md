# Semantic model

**Status:** conceptual draft  
**Related:** [type system](type-system.md), [failure model](failure-model.md),
[syntax](syntax.md), [assurance model](assurance-model.md),
[decision register](decision-register.md)

The semantic model matters more than the parser grammar. It defines the concepts
the compiler can reason about and the guarantees each concept carries. Surface
syntax should remain replaceable until complete example programs demonstrate
that these are the right abstraction boundaries.

## 1. Program

An application is a closed, analysable graph of declarations and relationships.
The compiler should be able to answer:

- which routes exist and why;
- what each route accepts and returns;
- who may call it and which records they may access;
- which entities and fields it reads or mutates;
- which external services and secrets it can use;
- which events/jobs it creates;
- which domain and operational failures can occur;
- which human-approved policies apply;
- which tests and generated checks support those claims.

Unknown behaviour should not hide inside framework callbacks or arbitrary code.

## 2. Core concepts

### 2.1 `app`

The application boundary gives declarations a shared identity and build target.
It owns global configuration, generated artifacts, deployment metadata, and the
complete route/effect graph.

The exact module/file relationship is open.

### 2.2 `type`

A type is an executable contract. It may include domain constraints in addition
to structure. One declaration may generate or inform:

- compile-time checks;
- runtime validators;
- Postgres column types and `CHECK` constraints;
- input and output schemas;
- OpenAPI and documentation;
- generated tests.

Typing inside trusted code means the value was actually validated, not merely
annotated.

Application semantic types are nominal rather than structurally compatible. A
shared primitive representation is an implementation fact, not permission to
substitute one domain value for another. Raw text therefore cannot stand in for
an email address, and one text-represented domain type cannot stand in for
another without an explicit, validated conversion.

`Type(value)` is a validated constructor for semantic and field types. A
constant argument is checked during compilation. A dynamic argument is checked
at runtime and introduces a typed validation failure that must be declared,
mapped, or handled. The form never means unchecked casting.

A constrained semantic type describes validated values, not a closed list of
named alternatives. For example, `InviteCode` is an open set of text values
that satisfy its constraints. If the domain has a finite state such as
`active`, `blocked`, `used`, or `expired`, that state belongs to an enum or
another explicit domain model. The language must not blur enum membership with
a distinguished or magic string value. Authored source references enum variants
only with qualified dot syntax such as `InviteStatus.blocked`; call-style forms
such as `InviteStatus("blocked")` or `InviteStatus(raw_text)` are invalid.
External representations enter through boundary decoding or an explicitly
typed parsing operation, not ordinary enum construction.

**Provisional:** an enum may later be a tagged sum whose variants carry
different typed payloads. A `paid` payment outcome can then require receipt and
time data while a `declined` outcome requires a decline reason, without placing
all of those fields into one nullable record. The active variant determines
which fields exist, and exhaustive matching provides the proof needed to use
them. This is intended to make invalid domain states unrepresentable, not to
add enum methods or presentation behaviour. Boundary tagging, payload
validation, persistence, database constraints, compatibility, and migration
semantics must be specified before such enums become implementable.

### 2.3 `input`

An input is untrusted data crossing into application code. Its declaration
distinguishes:

- required versus omitted fields;
- values versus explicit `none`;
- field and domain constraints;
- unknown/extra-field behaviour.

Successful validation produces a trusted typed value. Business code should not
repeat the same validation.

### 2.4 `output`

An output is the declared public shape leaving a boundary. The runtime validates
and serialises against that exact type.

Returning a richer internal value where a narrower public output is declared
must not accidentally expose extra fields. Either the compiler proves a safe
projection or the serializer rejects the mismatch.

### 2.5 `value`

A value is structured domain data without persistent entity identity. Examples
may include addresses, money breakdowns, date ranges, or provider contracts.

Its equality, copying, and serialisation semantics are open, but a concrete
value is complete: declared fields are never mysteriously missing.

### 2.6 `entity`

An entity is persistent, identified domain data. Initially it maps to Postgres.
Its declaration supplies:

- fields, nullability, defaults, and constraints;
- relationships and ownership-relevant references;
- indexes and uniqueness;
- migration implications;
- lifecycle consequences when fields or records change or disappear.

Fields are non-nullable by default. A nullable database column has language type
`T?`, with database `NULL` mapped to `none`.

A fetched entity is complete. Partial field selection produces a projection,
not a partially populated entity pretending to be complete.

Every field on a named record-like declaration introduces a semantic field type
referenced as `Type.field`, for example `Customer.id` or `Customer.email`. A
field value retains this identity rather than collapsing to its representation
primitive. This lets inputs, values, functions, actions, events, and other
declarations reuse a field contract without duplicating its validation rules.

A field type is a nominal refinement of its declared type. It can widen to that
declared type, but a base value cannot implicitly narrow to the field type and
sibling fields cannot be substituted for one another. Thus both
`Customer.email` and `Supplier.email` can be supplied where shared `Email` is
expected, while only the former satisfies a parameter typed `Customer.email`.

Field-type references carry value semantics and intrinsic validation, including
nullability. Presence or omission remains a property of the containing input or
object shape. References do not carry storage or stateful facts such as
defaults, indexes, uniqueness, referential existence, or authorisation. In
particular, validating a `Customer.id` does not claim that the customer exists,
is current, or is authorised for the actor. Loading it under application policy
produces a complete trusted `Customer`.

Code that wants interoperability across several fields names their shared
semantic type, such as `Email`. Code that names `Customer.email` deliberately
asks for the narrower field identity. This makes compatibility an authored
decision rather than an accident of structural typing.

When a field's declared parent is itself a named record, nested selection uses
that declared record's field identities. For example, if
`Customer.billing_address` refines `Address`, then
`customer.billing_address.postal_code` has type `Address.postal_code`, not a
synthetic `Customer.billing_address.postal_code`. Reusing `Address` deliberately
shares its nested contracts; distinct nested semantics require distinct named
record types. Nullable record fields must be handled before selection.

### 2.7 Projection

A projection is a complete type derived from an explicit subset or transformation
of data. It prevents the ORM-style state in which an object claims to be a
`Customer` while some fields were never selected.

The compiler may infer anonymous projection types or allow them to be named.

### 2.8 `query`

A query is a compiler-understood database read. The semantic model knows:

- entity/projection returned;
- cardinality: many, optional one, or required one;
- filters and ownership scope;
- ordering, limits, and pagination;
- whether policy permits the selected rows and fields;
- resource/cost bounds where knowable.

Raw SQL is not an ordinary query. Complex capabilities should first be added as
safe declarative query forms. Any raw escape hatch is explicitly audited.

### 2.9 `action`

An action is a domain operation capable of persistent or declared external
effects. It owns or makes visible:

- reads and mutations;
- authorisation preconditions;
- transaction boundaries;
- emitted events;
- service operations;
- typed domain failures;
- operational failure policy.

Every database mutation belongs to an action or an equivalently explicit route
body. The compiler must be able to see the action's effect graph.

The default boundary is the complete mutative action. The compiler infers this
transitively, passes one transaction-scoped persistence capability through
nested callable invocations, and reuses it if a nested mutative action is
entered. Reads within that action share the same transaction; read-only actions
avoid a write transaction. Explicit syntax is reserved for advanced choices
such as isolation or intentionally splitting a boundary, not for remembering
basic atomicity.

### 2.10 Function

A function performs ordinary computation. The design preference is to keep it
free from persistent and external effects so a call is locally understandable.

Whether functions may perform database reads is unresolved. Their effect rules
must be explicit enough that an agent never has to guess whether a call can
mutate state, perform network I/O, or access secrets.

The current design direction prohibits raw representation primitives in normal
application-defined callable signatures. Parameters and results use named
semantic types or field types. Primitives remain the substrate for type
declarations, local computation, operators, and low-level standard-library
facilities; whether authored low-level utilities need an explicit escape hatch
will be tested in the golden applications.

### 2.11 `route`

A route binds an HTTP transport boundary to typed behaviour. It declares or
inherits:

- method and path;
- authentication and authorisation;
- path/query/header/body input;
- action or local behaviour;
- output type and serialisation;
- mapping from domain failures to HTTP responses;
- rate/resource policy;
- generated and authored tests.

Public access is an explicit weakening of the default, not the result of an
omitted authentication line.

Authentication mechanism is not route business logic. Session cookies,
OIDC/JWT, API keys, service identities, and future strategies terminate at a
compiler-generated boundary that validates and normalises them into one typed
actor context. Routes inherit the requirement for an authenticated actor and
may describe stronger requirements or an explicit public exception; they do
not select a provider. Actions and policy see only stable identity, tenant,
allowlisted user information, permissions/capabilities, and authentication
strength. When several strategies are enabled, selection and conflicts are
deterministic and privileges are never combined implicitly.

### 2.12 `policy`

Policy is a human-owned statement of permitted behaviour. It can cover:

- actor and role permissions;
- record ownership and tenancy;
- field-level read/write rules;
- destructive lifecycle choices;
- public exposure;
- allowed external effects and exceptional escape hatches.

Implementation is checked against policy. Audit is derived from both. The agent
may not silently change policy to resolve a violation.

### 2.13 Intent, rule, or decision

The conversation identified a need to preserve why behaviour exists and which
choices required human approval. The semantic concept is accepted; its shape is
open.

Possible uses include:

- natural-language intent linked to a compiler-understood rule;
- durable design decisions and rationale;
- markers that a compiler diagnostic requires a human rather than an agent;
- traceability from policy to implementation and tests.

### 2.14 `event`

An event is an explicit fact emitted by an action. Its schema is typed. The
compiler knows which actions emit it and which handlers consume it.

Delivery, transaction/outbox behaviour, retention, idempotency, ordering, and
replay semantics are open but must be declared or safely defaulted.

### 2.15 `job`

A job is scheduled or background work. It declares schedule/trigger,
concurrency, resource bounds, effects, retries, idempotency, and failures.

Async work must not inherit accidental behaviour from a queue library. The
compiler should require unresolved delivery decisions explicitly.

### 2.16 `service`

A service is a reviewed external contract. It defines permitted operations,
schemas, authentication/secrets, timeouts, retries, idempotency, and provider
error mapping.

OpenAPI may be imported directly. Documentation without a machine-readable spec
may be converted by an LLM once, then reviewed and committed as a contract. From
that point on, use is typed and auditable rather than arbitrary network code.

Provider-specific failures are normalised at this boundary into domain failures
or operational faults.

### 2.17 `test`

A test is either:

- generated from semantics the compiler already knows, such as authentication,
  ownership, malformed IDs, constraints, and response shape; or
- authored to prove business intent that cannot be inferred.

Tests may live beside behaviour to preserve locality, while reusable fixtures
and broader scenarios still need a module-level design.

## 3. Trust model

### 3.1 Untrusted boundaries

Values are untrusted when entering from:

- HTTP input;
- HTTP output implementations before serialization;
- database reads;
- database writes;
- environment and configuration;
- queues and events;
- third-party API responses;
- deserialised caches.

Validation is built into the boundary. A boundary mismatch is a hard contract
failure, not a partially valid application value.

### 3.2 Trusted application values

Once a value has a declared application type, code may rely on that type and its
constraints. This is the invariant:

> Inside trusted application code, typed values are actually valid.

The model should prevent an LLM from wondering whether it needs another Zod
parse or defensive field check.

### 3.3 Absence and shape

The semantic model separates two facts:

- `T?` means a present field may contain a `T` or `none`;
- `optional` means an input/object-shape field may be omitted.

Omission is not stored as `undefined`. This distinction is essential for patch
operations, where omitted means “do not change” and explicit `none` means
“clear”.

## 4. Failure model

### 4.1 Domain failures

Expected business outcomes are named, declared, and exhaustively propagated or
handled: `NotOwner`, `BookingUnavailable`, `PaymentDeclined`, and similar.

They exist at the semantic level without forcing the source to carry a
`Result<T, E>` wrapper through every line.

Every application failure derives from one standard kind such as `NotFound`,
`Conflict`, or `Rejected`. The standard kind owns default boundary behaviour;
the application failure owns a stable public code and domain meaning. Domain
code does not choose raw HTTP statuses or construct error responses.

Public failure data is an explicit schema and empty by default. Diagnostic
context is separately declared, redacted, and never implicitly serialised to a
client. Expected failures use semantic propagation traces rather than native
stacks by default.

### 4.2 Operational faults

Infrastructure problems—database unavailability, timeouts, provider
misconfiguration, network failures—are classified and handled through runtime
or service policy. Business code does not implement ad hoc retry loops.

Some provider errors become domain failures; others may only become operational
faults. For example, a declined card is business-relevant, while invalid Stripe
credentials indicate a broken integration and should be logged/alerted rather
than shown as an ordinary checkout choice.

Operational faults retain an internal cause chain and low-level stack. Their
external response is generic unless a reviewed boundary contract maps the
condition into a declared domain failure.

### 4.3 Bugs and impossible states

Compiler/runtime defects and violated invariants are internal errors. They map
to enriched diagnostics and 500-class responses, not declared business flow.

Arbitrary generated-target exceptions are caught only at runtime containment
boundaries and reported as invariant breaches. They never become an untyped
application control-flow mechanism.

## 5. Data lifecycle semantics

Deleting an entity or field is not merely a generated migration. The compiler
must discover dependants and require an explicit lifecycle decision:

- cascade;
- retain;
- anonymise;
- reject;
- another deliberately modelled domain action.

The same principle applies when schema changes meet existing data. Migrations
are generated, but unresolved data meaning cannot be guessed.

## 6. Effect graph

The compiler maintains a graph connecting routes, actions, queries, mutations,
events, jobs, services, secrets, entities, fields, and policies. The graph
supports:

- authorisation proofs;
- secret-flow and output checks;
- declared-egress enforcement;
- transaction inference or validation;
- lifecycle impact analysis;
- route inventory and documentation;
- audit output;
- generated tests;
- human-review summaries.

The exact proof system is open, but this graph—not generated TypeScript—is the
core value of the compiler.

## 7. Locality and abstraction

The language should keep a small behaviour local rather than force it through
controller, service, repository, DTO, mapper, factory, and middleware layers.
A route may contain a small action and tests. Reusable domain behaviour can be
extracted to a named action or function.

Abstraction remains possible but should be more intentional than in TypeScript.
LLMs frequently invent layers that increase navigation and context without
adding domain meaning.

## 8. Unresolved semantic boundaries

Complete example programs must determine:

- module and visibility semantics;
- whether functions are pure and whether they may read persistence;
- advanced transaction isolation, savepoints, and external-effect interaction;
- query cardinality and absence behaviour;
- pagination and streaming;
- representation strategies for immutable values at large-data and foreign-
  buffer boundaries; copying, moving, sharing, and copy-on-write must remain
  unobservable, and caller-visible mutation remains absent until separately
  justified;
- relationship and ownership inference;
- policy expressiveness and proof rules;
- event/job delivery guarantees;
- service contract versioning;
- configuration and secret types;
- extension/escape-hatch boundaries;
- concurrency, resource, and execution limits;
- schema/data migration decisions;
- authored intent and decision semantics.
